//! Reducers of the font-data family (per source): `font.setInfo`, `font.setVerticalMetrics`,
//! `kerning.setPairs`, `kerning.removePairs`, `kerning.setGroup`, `features.setCode`.
//! Step 5 writes them; until then each fails with `E_NOT_IMPLEMENTED`.

use tf_commands::command::{
    FeaturesSetCode, FontSetInfo, FontSetVerticalMetrics, KerningRemovePairs, KerningSetGroup,
    KerningSetPairs,
};

not_implemented! {
    set_info(FontSetInfo) = "font.setInfo";
    set_vertical_metrics(FontSetVerticalMetrics) = "font.setVerticalMetrics";
    set_kerning_pairs(KerningSetPairs) = "kerning.setPairs";
    remove_kerning_pairs(KerningRemovePairs) = "kerning.removePairs";
    set_kerning_group(KerningSetGroup) = "kerning.setGroup";
    set_features(FeaturesSetCode) = "features.setCode";
}
