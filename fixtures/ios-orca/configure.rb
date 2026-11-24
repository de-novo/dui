# Loaded by the disposable Expo project's Podfile using CocoaPods' xcodeproj.
require 'xcodeproj'

project = Xcodeproj::Project.open('Orca.xcodeproj')
app = project.native_targets.fetch(0)
raise 'Expected Orca application' unless app.name == 'Orca'
tests = project.new_target(:ui_test_bundle, 'OrcaCapture', :ios, '17.0')
tests.add_dependency(app)
source = project.main_group.new_file(File.join(__dir__, 'Capture.swift'))
tests.add_file_references([source])
tests.build_configurations.each do |config|
  config.build_settings.merge!({
    'GENERATE_INFOPLIST_FILE' => 'YES',
    'PRODUCT_BUNDLE_IDENTIFIER' => 'org.denovo.dui.OrcaCapture',
    'SWIFT_VERSION' => '5.0',
    'TEST_TARGET_NAME' => app.name
  })
end
project.save
scheme = Xcodeproj::XCScheme.new
scheme.configure_with_targets(app, tests)
scheme.save_as(project.path, 'OrcaCapture', true)
