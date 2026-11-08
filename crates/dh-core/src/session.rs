use dh_auth::{AuthenticatedConnection, Binding, random_bytes};
use std::collections::BTreeSet;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostState {
    Ready { user: String },
    Locked,
    SignIn,
    PermissionNeeded,
    NoDisplay,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    View,
    Input,
    Clipboard,
    Files,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReleasedInput {
    pub keys: Vec<u16>,
    pub buttons: u32,
}
#[derive(Debug, PartialEq, Eq)]
pub enum PolicyError {
    Busy,
    Unauthorized,
    StaleEpoch,
    StaleLayout,
    PermissionDenied,
    Unavailable,
    InvalidInput,
}
pub struct Grant {
    token: [u8; 32],
    epoch: u64,
    layout: u64,
}
struct Controller {
    binding: Binding,
    token: [u8; 32],
    permissions: Vec<Operation>,
}
pub struct Broker {
    state: HostState,
    epoch: u64,
    layout: u64,
    controller: Option<Controller>,
    keys: BTreeSet<u16>,
    buttons: u32,
}
impl Broker {
    pub fn new(state: HostState) -> Self {
        Self {
            state,
            epoch: 1,
            layout: 1,
            controller: None,
            keys: BTreeSet::new(),
            buttons: 0,
        }
    }
    pub fn authorize(
        &mut self,
        proof: AuthenticatedConnection,
        permissions: Vec<Operation>,
    ) -> Result<Grant, PolicyError> {
        if self.controller.is_some() {
            return Err(PolicyError::Busy);
        }
        if permissions.is_empty() || permissions.len() > 4 {
            return Err(PolicyError::PermissionDenied);
        }
        let token = random_bytes();
        self.controller = Some(Controller {
            binding: proof.binding().clone(),
            token,
            permissions,
        });
        Ok(Grant {
            token,
            epoch: self.epoch,
            layout: self.layout,
        })
    }
    pub fn check(
        &self,
        grant: &Grant,
        binding: &Binding,
        operation: Operation,
        epoch: u64,
        layout: u64,
    ) -> Result<(), PolicyError> {
        let controller = self.controller.as_ref().ok_or(PolicyError::Unauthorized)?;
        if &controller.binding != binding || controller.token != grant.token {
            return Err(PolicyError::Unauthorized);
        }
        if epoch != self.epoch || grant.epoch != self.epoch {
            return Err(PolicyError::StaleEpoch);
        }
        if layout != self.layout || grant.layout != self.layout {
            return Err(PolicyError::StaleLayout);
        }
        if !controller.permissions.contains(&operation) {
            return Err(PolicyError::PermissionDenied);
        }
        match self.state {
            HostState::NoDisplay | HostState::PermissionNeeded => Err(PolicyError::Unavailable),
            HostState::SignIn | HostState::Locked
                if matches!(operation, Operation::Clipboard | Operation::Files) =>
            {
                Err(PolicyError::PermissionDenied)
            }
            _ => Ok(()),
        }
    }
    /// Only the broker supervising this live authenticated connection may renew its local grant.
    /// This is not a reconnect token; a new transport must authenticate again.
    pub fn refresh(&self, binding: &Binding) -> Result<Grant, PolicyError> {
        let controller = self
            .controller
            .as_ref()
            .filter(|c| &c.binding == binding)
            .ok_or(PolicyError::Unauthorized)?;
        Ok(Grant {
            token: controller.token,
            epoch: self.epoch,
            layout: self.layout,
        })
    }
    pub fn transition(&mut self, state: HostState) -> ReleasedInput {
        let release = self.release_all();
        self.epoch = self.epoch.checked_add(1).expect("session epoch exhausted");
        self.state = state;
        release
    }
    pub fn replace_layout(&mut self) -> ReleasedInput {
        let release = self.release_all();
        self.layout = self
            .layout
            .checked_add(1)
            .expect("display generation exhausted");
        release
    }
    pub fn disconnect(&mut self, binding: &Binding) -> ReleasedInput {
        if self
            .controller
            .as_ref()
            .is_some_and(|c| &c.binding == binding)
        {
            self.controller = None;
            return self.release_all();
        }
        ReleasedInput::default()
    }
    pub fn revoke_all(&mut self) -> ReleasedInput {
        self.controller = None;
        self.epoch = self.epoch.checked_add(1).expect("session epoch exhausted");
        self.release_all()
    }
    pub fn key(
        &mut self,
        grant: &Grant,
        binding: &Binding,
        epoch: u64,
        layout: u64,
        usage: u16,
        pressed: bool,
    ) -> Result<(), PolicyError> {
        self.check(grant, binding, Operation::Input, epoch, layout)?;
        if !(4..=0xe7).contains(&usage) {
            return Err(PolicyError::InvalidInput);
        }
        if pressed {
            self.keys.insert(usage);
        } else {
            self.keys.remove(&usage);
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)] // Wire pointer components share one authorization check.
    pub fn pointer(
        &mut self,
        grant: &Grant,
        binding: &Binding,
        epoch: u64,
        layout: u64,
        x: f64,
        y: f64,
        buttons: u32,
    ) -> Result<(), PolicyError> {
        self.check(grant, binding, Operation::Input, epoch, layout)?;
        if !x.is_finite()
            || !y.is_finite()
            || !(0.0..=1.0).contains(&x)
            || !(0.0..=1.0).contains(&y)
            || buttons & !31 != 0
        {
            return Err(PolicyError::InvalidInput);
        }
        self.buttons = buttons;
        Ok(())
    }
    pub fn release_all(&mut self) -> ReleasedInput {
        ReleasedInput {
            keys: std::mem::take(&mut self.keys).into_iter().collect(),
            buttons: std::mem::take(&mut self.buttons),
        }
    }
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
    pub fn layout(&self) -> u64 {
        self.layout
    }
    pub fn state(&self) -> &HostState {
        &self.state
    }
}
