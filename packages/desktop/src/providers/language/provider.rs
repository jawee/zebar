use std::{sync::Arc, thread, time::Duration};

use anyhow::bail;
use async_trait::async_trait;
use tokio::{
  sync::mpsc::Sender,
  task::{self, AbortHandle},
};
use tracing::info;
use windows::{
  core::w,
  Win32::{
    Devices::HumanInterfaceDevice::{
      HID_USAGE_GENERIC_MOUSE, HID_USAGE_PAGE_GENERIC,
    },
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Globalization::{LCIDToLocaleName, LOCALE_ALLOW_NEUTRAL_NAMES},
    System::SystemServices::LOCALE_NAME_MAX_LENGTH,
    UI::{
      Input::{RegisterRawInputDevices, RAWINPUTDEVICE, RIDEV_INPUTSINK},
      WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW,
        RegisterClassW, TranslateMessage, CS_HREDRAW, CS_VREDRAW,
        CW_USEDEFAULT, MSG, SHOW_WINDOW_CMD, WINDOW_EX_STYLE,
        WM_INPUTLANGCHANGE, WNDCLASSW, WS_OVERLAPPEDWINDOW,
      },
    },
  },
};

use super::{
  language::{Language, WindowProcedure},
  LanguageProviderConfig, LanguageVariables,
};
use crate::providers::{provider::Provider, provider_ref::ProviderOutput};

pub struct LanguageProvider {
  pub config: Arc<LanguageProviderConfig>,
  abort_handle: Option<AbortHandle>,
  state: Arc<Language>,
}

impl LanguageProvider {
  pub fn new(config: LanguageProviderConfig) -> LanguageProvider {
    LanguageProvider {
      config: Arc::new(config),
      abort_handle: None,
      state: Language::new().into(),
    }
  }
}

pub fn create_message_window(
  window_procedure: WindowProcedure,
) -> anyhow::Result<isize> {
  let wnd_class = WNDCLASSW {
    lpszClassName: w!("MessageWindow"),
    style: CS_HREDRAW | CS_VREDRAW,
    lpfnWndProc: window_procedure,
    ..Default::default()
  };

  unsafe { RegisterClassW(&wnd_class) };

  let handle = unsafe {
    CreateWindowExW(
      Default::default(),
      w!("MessageWindow"),
      w!("MessageWindow"),
      WS_OVERLAPPEDWINDOW,
      CW_USEDEFAULT,
      CW_USEDEFAULT,
      CW_USEDEFAULT,
      CW_USEDEFAULT,
      None,
      None,
      wnd_class.hInstance,
      None,
    )
  };

  if handle.0 == 0 {
    bail!("Creation of message window failed.");
  }

  Ok(handle.0)
}

pub fn run_message_loop() {
  let mut msg = MSG::default();

  loop {
    if unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
      unsafe {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
      }
    } else {
      break;
    }
  }
}

// let window_thread = thread::spawn(move || {
//       // Start hooks for listening to platform events.
//       keyboard_hook_clone.start()?;
//       window_event_hook.start()?;
//
//       // Create a hidden window with a message loop on the current
// thread.       let handle =
//         Platform::create_message_window(Some(event_window_proc))?;
//
//       let rid = RAWINPUTDEVICE {
//         usUsagePage: HID_USAGE_PAGE_GENERIC,
//         usUsage: HID_USAGE_GENERIC_MOUSE,
//         dwFlags: RIDEV_INPUTSINK,
//         hwndTarget: HWND(handle),
//       };
//
//       // Register our window to receive mouse events.
//       unsafe {
//         RegisterRawInputDevices(
//           &[rid],
//           std::mem::size_of::<RAWINPUTDEVICE>() as u32,
//         )
//       }?;
//
//       Platform::run_message_loop();
//
//       // Clean-up on message loop exit.
//       unsafe { DestroyWindow(HWND(handle)) }?;
//       keyboard_hook_clone.stop()?;
//       window_event_hook.stop()?;
//
//       Ok(())
//     });
extern "system" fn wndproc(
  hwnd: HWND,
  msg: u32,
  wparam: WPARAM,
  lparam: LPARAM,
) -> LRESULT {
  match msg {
    WM_INPUTLANGCHANGE => {
      let lang_id = loword(lparam.0);

      let mut locale_name = [0; LOCALE_NAME_MAX_LENGTH as usize];

      let result = unsafe {
        LCIDToLocaleName(
          lang_id,
          Some(&mut locale_name),
          LOCALE_ALLOW_NEUTRAL_NAMES,
        )
      };

      if result > 0 {
        let actual_name =
          String::from_utf16_lossy(&locale_name[..result as usize]);
        println!("{}", actual_name);
        info!("{}", actual_name);
      } else {
        info!("Failed to parse language. lang_id: {}", lang_id);
      }
    }
    _ => {
      info!("hello from other message");
    }
  }

  unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

pub fn loword(l: isize) -> u32 {
  return (l as u32) & 0xffff;
}

#[async_trait]
impl Provider for LanguageProvider {
  async fn on_start(
    &mut self,
    config_hash: &str,
    emit_output_tx: Sender<ProviderOutput>,
  ) {
    let window_thread = task::spawn(async move {
      // Create window

      let handle = create_message_window(Some(wndproc)).unwrap();

       let rid = RAWINPUTDEVICE {
        usUsagePage: HID_USAGE_PAGE_GENERIC,
        usUsage: HID_USAGE_GENERIC_MOUSE,
        dwFlags: RIDEV_INPUTSINK,
        hwndTarget: HWND(handle),
      };

      unsafe {
        RegisterRawInputDevices(
          &[rid],
          std::mem::size_of::<RAWINPUTDEVICE>() as u32,
        )
      }.unwrap();
      // Message loop
      run_message_loop();
    });

    info!("Started thread");
    self.abort_handle = Some(window_thread.abort_handle());
    _ = window_thread.await;
  }

  /// Callback for when the provider is refreshed.
  async fn on_refresh(
    &mut self,
    config_hash: &str,
    emit_output_tx: Sender<ProviderOutput>,
  ) {
    // todo!("on_refresh");
    info!("on_refresh called in LanguageProvider");
  }

  /// Callback for when the provider is stopped.
  async fn on_stop(&mut self) {
    // todo!("on_stop");
    info!("on_stop called in LanguageProvider");
    if let Some(handle) = &self.abort_handle {
      handle.abort();
    }
  }

  /// Minimum interval between refreshes.
  ///
  /// Affects how the provider output is cached.
  fn min_refresh_interval(&self) -> Option<Duration> {
    None
  }
}
