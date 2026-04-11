use core_foundation::{
    array::{CFArray, CFArrayRef},
    base::{CFType, CFTypeRef, TCFType},
    boolean::CFBoolean,
    dictionary::{CFDictionary, CFDictionaryRef},
    string::{CFString, CFStringRef},
};
use objc2_foundation::{NSProcessInfo, NSProcessInfoThermalState};

use crate::{Error, PowerSource, Snapshot, ThermalState};

const AC_POWER_VALUE: &str = "AC Power";
const BATTERY_POWER_VALUE: &str = "Battery Power";
const IS_CHARGING_KEY: &str = "Is Charging";

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPSCopyPowerSourcesInfo() -> CFTypeRef;
    fn IOPSCopyPowerSourcesList(blob: CFTypeRef) -> CFArrayRef;
    fn IOPSGetPowerSourceDescription(blob: CFTypeRef, source: CFTypeRef) -> CFDictionaryRef;
    fn IOPSGetProvidingPowerSourceType(blob: CFTypeRef) -> CFStringRef;
}

pub fn snapshot() -> Result<Snapshot, Error> {
    let blob_ref = unsafe { IOPSCopyPowerSourcesInfo() };
    if blob_ref.is_null() {
        return Err(Error::Unavailable("IOPSCopyPowerSourcesInfo"));
    }

    let blob = unsafe { CFType::wrap_under_create_rule(blob_ref) };
    let descriptions = power_source_descriptions(&blob)?;
    let process_info = NSProcessInfo::processInfo();

    Ok(Snapshot {
        has_battery: !descriptions.is_empty(),
        power_source: providing_power_source(&blob)?,
        is_charging: descriptions
            .iter()
            .find_map(|description| find_bool(description, IS_CHARGING_KEY)),
        low_power_mode: process_info.isLowPowerModeEnabled(),
        thermal_state: thermal_state(process_info.thermalState()),
    })
}

fn power_source_descriptions(blob: &CFType) -> Result<Vec<CFDictionary<CFString, CFType>>, Error> {
    let list_ref = unsafe { IOPSCopyPowerSourcesList(blob.as_CFTypeRef()) };
    if list_ref.is_null() {
        return Err(Error::Unavailable("IOPSCopyPowerSourcesList"));
    }

    let list = unsafe { CFArray::<CFType>::wrap_under_create_rule(list_ref) };
    let mut descriptions = Vec::with_capacity(list.len() as usize);

    for index in 0..list.len() {
        let Some(source) = list.get(index) else {
            continue;
        };

        let description_ref =
            unsafe { IOPSGetPowerSourceDescription(blob.as_CFTypeRef(), source.as_CFTypeRef()) };
        if description_ref.is_null() {
            continue;
        }

        descriptions.push(unsafe { CFDictionary::wrap_under_get_rule(description_ref) });
    }

    Ok(descriptions)
}

fn providing_power_source(blob: &CFType) -> Result<PowerSource, Error> {
    let value_ref = unsafe { IOPSGetProvidingPowerSourceType(blob.as_CFTypeRef()) };
    if value_ref.is_null() {
        return Err(Error::Unavailable("IOPSGetProvidingPowerSourceType"));
    }

    let value = unsafe { CFString::wrap_under_get_rule(value_ref) }.to_string();
    Ok(match value.as_str() {
        AC_POWER_VALUE => PowerSource::Ac,
        BATTERY_POWER_VALUE => PowerSource::Battery,
        _ => PowerSource::Unknown,
    })
}

fn find_bool(description: &CFDictionary<CFString, CFType>, key: &str) -> Option<bool> {
    description
        .find(CFString::new(key))
        .and_then(|value| value.downcast::<CFBoolean>())
        .map(bool::from)
}

fn thermal_state(value: NSProcessInfoThermalState) -> ThermalState {
    match value {
        NSProcessInfoThermalState::Nominal => ThermalState::Nominal,
        NSProcessInfoThermalState::Fair => ThermalState::Fair,
        NSProcessInfoThermalState::Serious => ThermalState::Serious,
        NSProcessInfoThermalState::Critical => ThermalState::Critical,
        _ => ThermalState::Unknown,
    }
}
