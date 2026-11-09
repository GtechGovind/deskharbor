use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use zeroize::Zeroizing;
const PASS: &[u8] = b"synthetic-review-password";
struct Scene {
    count: AtomicUsize,
    fail: AtomicBool,
}
impl DesktopSource for Arc<Scene> {
    fn observe(&self) -> (HostState, Vec<Display>, String) {
        let displays = (0..self.count.load(Ordering::Acquire))
            .map(|i| Display {
                id: format!("synthetic-{i}"),
                name: format!("Synthetic {i}"),
                width: 32 + 16 * i as u32,
                height: 32,
                x: 0,
                y: 0,
            })
            .collect();
        (
            HostState::Ready {
                user: "synthetic-review-user".into(),
            },
            displays,
            "Synthetic review backend".into(),
        )
    }
    fn capture(&self, _: &str, w: u32, h: u32, _: u64) -> EngineResult<Vec<u8>> {
        if self.fail.load(Ordering::Acquire) {
            return Err(err("codec", "Synthetic capture failure"));
        }
        Ok(vec![120; (w * h * 4) as usize])
    }
}
struct Peer {
    scene: Arc<Scene>,
    host: Arc<Host>,
    addr: String,
    listener: tokio::task::JoinHandle<()>,
}
impl Peer {
    async fn start(count: usize) -> Self {
        let scene = Arc::new(Scene {
            count: AtomicUsize::new(count),
            fail: AtomicBool::new(false),
        });
        let mut host = Host::new(
            HostBackend::Synthetic,
            HostSettings {
                clipboard: true,
                ..Default::default()
            },
            HostCredentials::register(PASS).unwrap(),
            1,
            None,
        );
        host.source = Box::new(scene.clone());
        let host = Arc::new(host);
        let listener = dh_net::Listener::bind(
            "127.0.0.1:0".parse().unwrap(),
            dh_net::Identity::generate().unwrap(),
            true,
        )
        .await
        .unwrap();
        let addr = listener.local_addr().to_string();
        let h = host.clone();
        let task = tokio::spawn(async move {
            let mut workers = tokio::task::JoinSet::new();
            loop {
                let connecting = listener.incoming().await.unwrap();
                let host = h.clone();
                workers.spawn(async move {
                    if let Ok(ch) = connecting.await {
                        let _ = serve(host, ch).await;
                    }
                });
            }
        });
        Self {
            scene,
            host,
            addr,
            listener: task,
        }
    }
    async fn connect(&self, e: &DeskEngine) -> SessionInfo {
        e.connect(self.addr.clone(), Zeroizing::new(PASS.to_vec()))
            .await
            .unwrap()
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.listener.abort()
    }
}
async fn next_event(
    events: &mut broadcast::Receiver<EngineEvent>,
    want: impl Fn(&EngineEvent) -> bool,
) -> EngineEvent {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let event = events.recv().await.unwrap();
            if want(&event) {
                break event;
            }
        }
    })
    .await
    .expect("expected live session event")
}
#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn review_failed_media_blanks_until_a_decoded_frame_recovers_it() {
    let temp = tempfile::tempdir().unwrap();
    let client = DeskEngine::new(temp.path().into()).unwrap();
    let peer = Peer::start(1).await;
    let mut events = client.subscribe();
    let session = peer.connect(&client).await;
    let EngineEvent::FrameReady {
        sequence: first, ..
    } = next_event(&mut events, |e| matches!(e, EngineEvent::FrameReady { .. })).await
    else {
        unreachable!()
    };
    assert!(client.latest_frame(session.id).is_some());
    peer.scene.fail.store(true, Ordering::Release);
    next_event(
        &mut events,
        |e| matches!(e,EngineEvent::SessionChanged(s) if s.id==session.id && s.state=="no_display"),
    )
    .await;
    next_event(
        &mut events,
        |e| matches!(e,EngineEvent::Error{session_id:Some(id),..} if *id==session.id),
    )
    .await;
    assert!(
        client.latest_frame(session.id).is_none(),
        "failed stream retained its last image"
    );
    client
        .clipboard_send_text(session.id, "still authenticated".into())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert_eq!(
        client
            .session(session.id)
            .unwrap()
            .info
            .lock()
            .unwrap()
            .state,
        "no_display",
        "ready heartbeat masked media failure"
    );
    peer.scene.fail.store(false, Ordering::Release);
    next_event(
        &mut events,
        |e| matches!(e,EngineEvent::FrameReady{sequence,..} if *sequence>first),
    )
    .await;
    assert_eq!(
        client
            .session(session.id)
            .unwrap()
            .info
            .lock()
            .unwrap()
            .state,
        "connected"
    );
    client.disconnect(session.id).await.unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn review_topology_and_fresh_connections_use_the_hosts_selected_monitor() {
    let temp = tempfile::tempdir().unwrap();
    let client = DeskEngine::new(temp.path().into()).unwrap();
    let peer = Peer::start(2).await;
    let mut events = client.subscribe();
    let session = peer.connect(&client).await;
    client
        .select_monitor(session.id, "synthetic-1".into())
        .await
        .unwrap();
    next_event(&mut events, |e| {
        matches!(e, EngineEvent::FrameReady { width: 48, .. })
    })
    .await;
    peer.scene.count.store(3, Ordering::Release);
    let EngineEvent::SessionChanged(topology) = next_event(
        &mut events,
        |e| matches!(e,EngineEvent::SessionChanged(s) if s.displays.len()==3),
    )
    .await
    else {
        unreachable!()
    };
    let selected = peer.host.refresh().selected.clone();
    assert_eq!(
        topology.selected_display, selected,
        "client retained a different monitor after hotplug"
    );
    let width = topology
        .displays
        .iter()
        .find(|d| d.id == selected)
        .unwrap()
        .width;
    next_event(
        &mut events,
        |e| matches!(e,EngineEvent::FrameReady{width:w,..} if *w==width),
    )
    .await;
    client
        .select_monitor(session.id, "synthetic-2".into())
        .await
        .unwrap();
    client.disconnect(session.id).await.unwrap();
    let fresh = peer.connect(&client).await;
    assert_eq!(
        fresh.selected_display,
        peer.host.refresh().selected,
        "fresh connection picked a different monitor"
    );
    next_event(
        &mut events,
        |e| matches!(e,EngineEvent::FrameReady{session_id,width:64,..} if *session_id==fresh.id),
    )
    .await;
    client.disconnect(fresh.id).await.unwrap();
}
