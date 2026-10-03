//! Windows 10 and 11 through QueryDisplayConfig and SetDisplayConfig.

use std::mem::size_of;

use windows::Win32::Devices::Display::{
    DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig, SetDisplayConfig,
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
    DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
    SDC_ALLOW_CHANGES, SDC_APPLY, SDC_SAVE_TO_DATABASE, SDC_USE_SUPPLIED_DISPLAY_CONFIG,
    SDC_VALIDATE,
};
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS};

use super::ccd::{screen_id, to_state, CcdPath};
use super::{ApplyMode, Backend, Capabilities};
use crate::layout::Origin;
use crate::model::{Layout, Rect, State};
use crate::Error;

pub struct Windows;

/// The bit in DISPLAYCONFIG_TARGET_DEVICE_NAME_FLAGS saying the EDID ids are valid.
const EDID_IDS_VALID: u32 = 0x4;

fn wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn query_raw() -> Result<(Vec<DISPLAYCONFIG_PATH_INFO>, Vec<DISPLAYCONFIG_MODE_INFO>), Error> {
    loop {
        let (mut np, mut nm) = (0u32, 0u32);
        let rc = unsafe { GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm) };
        if rc != ERROR_SUCCESS {
            return Err(Error::System(format!(
                "GetDisplayConfigBufferSizes failed ({}).",
                rc.0
            )));
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); np as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); nm as usize];
        let rc = unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut np,
                paths.as_mut_ptr(),
                &mut nm,
                modes.as_mut_ptr(),
                None,
            )
        };
        if rc == ERROR_INSUFFICIENT_BUFFER {
            // A screen arrived between the two calls.
            continue;
        }
        if rc != ERROR_SUCCESS {
            return Err(Error::System(format!(
                "QueryDisplayConfig failed ({}).",
                rc.0
            )));
        }
        paths.truncate(np as usize);
        modes.truncate(nm as usize);
        return Ok((paths, modes));
    }
}

fn source_mode_index(
    path: &DISPLAYCONFIG_PATH_INFO,
    modes: &[DISPLAYCONFIG_MODE_INFO],
) -> Option<usize> {
    let idx = unsafe { path.sourceInfo.Anonymous.modeInfoIdx } as usize;
    (idx < modes.len() && modes[idx].infoType == DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE).then_some(idx)
}

fn describe(path: &DISPLAYCONFIG_PATH_INFO, modes: &[DISPLAYCONFIG_MODE_INFO]) -> CcdPath {
    let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
    target.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
    target.header.size = size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
    target.header.adapterId = path.targetInfo.adapterId;
    target.header.id = path.targetInfo.id;
    let target_ok = unsafe { DisplayConfigGetDeviceInfo(&mut target.header) } == 0;

    let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
    source.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
    source.header.size = size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
    source.header.adapterId = path.sourceInfo.adapterId;
    source.header.id = path.sourceInfo.id;
    let source_ok = unsafe { DisplayConfigGetDeviceInfo(&mut source.header) } == 0;

    let rect = source_mode_index(path, modes).map(|i| {
        let m = unsafe { modes[i].Anonymous.sourceMode };
        Rect::new(m.position.x, m.position.y, m.width as i32, m.height as i32)
    });
    let edid_valid = target_ok && unsafe { target.flags.Anonymous.value } & EDID_IDS_VALID != 0;
    CcdPath {
        adapter_low: path.targetInfo.adapterId.LowPart,
        adapter_high: path.targetInfo.adapterId.HighPart,
        source_id: path.sourceInfo.id,
        target_id: path.targetInfo.id,
        technology: path.targetInfo.outputTechnology.0,
        source: rect,
        gdi_name: if source_ok {
            wide(&source.viewGdiDeviceName)
        } else {
            String::new()
        },
        friendly_name: if target_ok {
            wide(&target.monitorFriendlyDeviceName)
        } else {
            String::new()
        },
        edid: edid_valid.then_some((target.edidManufactureId, target.edidProductCodeId)),
    }
}

impl Backend for Windows {
    fn name(&self) -> &'static str {
        "windows"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            primary: true,
            temporary: true,
            verify: true,
            remembers: true,
            origin: Origin::Primary,
        }
    }

    fn query(&self) -> Result<State, Error> {
        let (paths, modes) = query_raw()?;
        Ok(to_state(
            &paths
                .iter()
                .map(|p| describe(p, &modes))
                .collect::<Vec<_>>(),
        ))
    }

    fn apply(&self, layout: &Layout, mode: ApplyMode) -> Result<(), Error> {
        let (paths, mut modes) = query_raw()?;
        let described: Vec<CcdPath> = paths.iter().map(|p| describe(p, &modes)).collect();
        let state = to_state(&described);
        let mut on: Vec<&str> = state.enabled().map(|s| s.id.as_str()).collect();
        let mut named: Vec<&str> = layout.positions.iter().map(|p| p.id.as_str()).collect();
        on.sort_unstable();
        named.sort_unstable();
        if on != named {
            return Err(Error::Changed);
        }
        for (path, info) in paths.iter().zip(&described) {
            let (Some(i), Some(p)) = (
                source_mode_index(path, &modes),
                layout.position(&screen_id(info)),
            ) else {
                continue;
            };
            // Writing a union field is safe; only reading it is not.
            modes[i].Anonymous.sourceMode.position.x = p.x;
            modes[i].Anonymous.sourceMode.position.y = p.y;
        }
        let flags = match mode {
            ApplyMode::Verify => SDC_VALIDATE | SDC_USE_SUPPLIED_DISPLAY_CONFIG,
            ApplyMode::Temporary => SDC_APPLY | SDC_USE_SUPPLIED_DISPLAY_CONFIG | SDC_ALLOW_CHANGES,
            ApplyMode::Persistent => {
                SDC_APPLY
                    | SDC_USE_SUPPLIED_DISPLAY_CONFIG
                    | SDC_ALLOW_CHANGES
                    | SDC_SAVE_TO_DATABASE
            }
        };
        let rc = unsafe { SetDisplayConfig(Some(&paths), Some(&modes), flags) };
        if rc != 0 {
            return Err(Error::System(format!(
                "Windows refused the arrangement (error {rc})."
            )));
        }
        Ok(())
    }
}
