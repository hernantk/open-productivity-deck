use serde::Serialize;
use std::ptr;
use windows::{
    core::Result as WindowsResult,
    Win32::{
        Foundation::RPC_E_CHANGED_MODE,
        Media::Audio::{
            eMultimedia, eRender, Endpoints::IAudioEndpointVolume, IMMDeviceEnumerator,
            MMDeviceEnumerator,
        },
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
        },
    },
};

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioState {
    pub volume: f32,
    pub muted: bool,
}

struct ComGuard {
    uninitialize: bool,
}

impl ComGuard {
    fn initialize() -> WindowsResult<Self> {
        let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if result.is_ok() {
            Ok(Self { uninitialize: true })
        } else if result == RPC_E_CHANGED_MODE {
            Ok(Self { uninitialize: false })
        } else {
            Err(result.into())
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.uninitialize {
            unsafe { CoUninitialize() };
        }
    }
}

fn with_endpoint<T>(operation: impl FnOnce(&IAudioEndpointVolume) -> WindowsResult<T>) -> WindowsResult<T> {
    let _com = ComGuard::initialize()?;
    let enumerator: IMMDeviceEnumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
    let device = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)? };
    let endpoint: IAudioEndpointVolume = unsafe { device.Activate(CLSCTX_ALL, None)? };
    operation(&endpoint)
}

pub fn state() -> Result<AudioState, String> {
    with_endpoint(|endpoint| unsafe {
        Ok(AudioState {
            volume: endpoint.GetMasterVolumeLevelScalar()?,
            muted: endpoint.GetMute()?.as_bool(),
        })
    })
    .map_err(|error| format!("O dispositivo de saída não está disponível: {error}"))
}

pub fn set_volume(value: f32) -> Result<AudioState, String> {
    let value = value.clamp(0.0, 1.0);
    with_endpoint(|endpoint| unsafe {
        endpoint.SetMasterVolumeLevelScalar(value, ptr::null())?;
        if endpoint.GetMute()?.as_bool() && value > 0.0 {
            endpoint.SetMute(false, ptr::null())?;
        }
        Ok(())
    })
    .map_err(|error| format!("Não foi possível alterar o volume: {error}"))?;
    state()
}

pub fn toggle_mute() -> Result<AudioState, String> {
    with_endpoint(|endpoint| unsafe {
        endpoint.SetMute(!endpoint.GetMute()?.as_bool(), ptr::null())
    })
    .map_err(|error| format!("Não foi possível alterar o mute: {error}"))?;
    state()
}
