//! Windows SAPI fallback (ISpVoice): used while Piper is not installed. It
//! speaks through the default output device itself, so it has no WAV step.

use super::Cancel;

#[cfg(windows)]
pub fn speak(text: &str, cancel: &Cancel) -> Result<(), String> {
    use std::time::Duration;

    use windows::Win32::Media::Speech::{
        ISpVoice, SPF_ASYNC, SPF_IS_NOT_XML, SPF_PURGEBEFORESPEAK, SPRS_DONE, SPVOICESTATUS,
        SpVoice,
    };
    use windows::Win32::System::Com::{
        CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
    };
    use windows::core::HSTRING;

    // SAFETY: COM calls on this thread only; the voice is dropped before the
    // matching CoUninitialize.
    unsafe {
        let init = CoInitializeEx(None, COINIT_MULTITHREADED);
        if init.is_err() {
            return Err(format!("could not initialise COM: {init:?}"));
        }
        let result = (|| -> Result<(), String> {
            let voice: ISpVoice = CoCreateInstance(&SpVoice, None, CLSCTX_ALL)
                .map_err(|e| format!("SAPI is unavailable: {e}"))?;
            voice
                .Speak(
                    &HSTRING::from(text),
                    (SPF_ASYNC.0 | SPF_IS_NOT_XML.0) as u32,
                    None,
                )
                .map_err(|e| format!("SAPI could not speak: {e}"))?;
            let tick = Duration::from_millis(40);
            loop {
                if cancel.is_cancelled() {
                    let _ = voice.Speak(
                        &HSTRING::new(),
                        (SPF_ASYNC.0 | SPF_PURGEBEFORESPEAK.0) as u32,
                        None,
                    );
                    return Ok(());
                }
                // WaitUntilDone returns Ok for its S_FALSE timeout too, so
                // it only paces the loop; the status says when speech ended.
                let _ = voice.WaitUntilDone(tick.as_millis() as u32);
                let mut status = SPVOICESTATUS::default();
                voice
                    .GetStatus(&mut status, std::ptr::null_mut())
                    .map_err(|e| format!("SAPI status failed: {e}"))?;
                if status.dwRunningState & (SPRS_DONE.0 as u32) != 0 {
                    return Ok(());
                }
            }
        })();
        CoUninitialize();
        result
    }
}

#[cfg(not(windows))]
pub fn speak(_text: &str, _cancel: &Cancel) -> Result<(), String> {
    Err("the SAPI fallback exists only on Windows".into())
}
