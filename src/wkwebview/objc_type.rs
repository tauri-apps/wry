// TODO: This file is for convinience testing the media permission and should be removed

#[repr(isize)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WKMediaCaptureType {
  Camera = 0,
  Microphone,
  CameraAndMicrophone,
}

impl From<isize> for WKMediaCaptureType {
  fn from(value: isize) -> Self {
    match value {
      0 => WKMediaCaptureType::Camera,
      1 => WKMediaCaptureType::Microphone,
      2 => WKMediaCaptureType::CameraAndMicrophone,
      _ => panic!("Invalid WKMediaCaptureType value"),
    }
  }
}

impl From<WKMediaCaptureType> for isize {
  fn from(value: WKMediaCaptureType) -> Self {
    match value {
      WKMediaCaptureType::Camera => 0,
      WKMediaCaptureType::Microphone => 1,
      WKMediaCaptureType::CameraAndMicrophone => 2,
    }
  }
}

#[repr(isize)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WKPermissionDecision {
  Prompt = 0,
  Grant,
  Deny,
}

impl From<isize> for WKPermissionDecision {
  fn from(value: isize) -> Self {
    match value {
      0 => WKPermissionDecision::Prompt,
      1 => WKPermissionDecision::Grant,
      2 => WKPermissionDecision::Deny,
      _ => panic!("Invalid WKPermissionDecision value"),
    }
  }
}

impl From<WKPermissionDecision> for isize {
  fn from(value: WKPermissionDecision) -> Self {
    match value {
      WKPermissionDecision::Prompt => 0,
      WKPermissionDecision::Grant => 1,
      WKPermissionDecision::Deny => 2,
    }
  }
}

#[repr(isize)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WKDisplayCapturePermissionDecision {
  Deny = 0,
  ScreenPrompt,
  WindowPrompt,
}

impl From<isize> for WKDisplayCapturePermissionDecision {
  fn from(value: isize) -> Self {
    match value {
      0 => WKDisplayCapturePermissionDecision::Deny,
      1 => WKDisplayCapturePermissionDecision::ScreenPrompt,
      2 => WKDisplayCapturePermissionDecision::WindowPrompt,
      _ => panic!("Invalid WKDisplayCapturePermissionDecision value"),
    }
  }
}

impl From<WKDisplayCapturePermissionDecision> for isize {
  fn from(value: WKDisplayCapturePermissionDecision) -> Self {
    match value {
      WKDisplayCapturePermissionDecision::Deny => 0,
      WKDisplayCapturePermissionDecision::ScreenPrompt => 1,
      WKDisplayCapturePermissionDecision::WindowPrompt => 2,
    }
  }
}
