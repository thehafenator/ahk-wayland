use crate::ahk::types::*;
use crate::hotstring::HotstringMatch;

pub fn transpile_to_xremap(_ahk: AhkConfig) -> AhkConfig {
    _ahk
}

pub fn extract_hotstrings(ahk: &AhkConfig) -> Vec<HotstringMatch> {
    ahk.hotstrings
        .iter()
        .enumerate()
        .map(|(idx, hs)| {
            HotstringMatch::from_trigger(
                idx,
                &hs.trigger,
                hs.replacement.clone(),
                hs.immediate,
                hs.case_sensitive,
                hs.omit_char,
                hs.execute,
            )
        })
        .collect()
}
