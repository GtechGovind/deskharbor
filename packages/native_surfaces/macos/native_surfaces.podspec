Pod::Spec.new do |s|
  s.name = 'native_surfaces'
  s.version = '0.0.1'
  s.summary = 'DeskHarbor native video texture boundary'
  s.description = 'Native RGBA frame leases from the shared DeskHarbor engine.'
  s.homepage = 'https://github.com/deskharbor/deskharbor'
  s.license = { :type => 'MIT', :file => '../LICENSE' }
  s.author = { 'DeskHarbor' => 'dev@deskharbor.invalid' }
  s.source = { :path => '.' }
  s.source_files = 'Classes/**/*', '../common/dh_frame.h', '../common/dh_copy.h', '../common/dh_loader.h', '../common/dh_library_path.h'
  s.public_header_files = 'Classes/NativeSurfacesPlugin.h'
  s.private_header_files = '../common/dh_frame.h', '../common/dh_copy.h', '../common/dh_loader.h', '../common/dh_library_path.h'
  s.dependency 'FlutterMacOS'
  s.platform = :osx, '10.15'
  s.frameworks = 'CoreVideo'
  s.requires_arc = true
  s.pod_target_xcconfig = { 'DEFINES_MODULE' => 'YES' }
end
