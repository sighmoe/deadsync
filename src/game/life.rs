pub const REGEN_COMBO_AFTER_MISS: u32 = 5;

// In SM, life regeneration is tied to LifePercentChangeHeld. Simply Love sets
// TimingWindowSecondsHold to 0.32s, so mirror that grace window. Reference:
// itgmania/Themes/Simply Love/Scripts/SL_Init.lua

pub struct LifeChange;
impl LifeChange {
    pub const FANTASTIC: f32 = 0.008;
    pub const EXCELLENT: f32 = 0.008;
    pub const GREAT: f32 = 0.004;
    pub const DECENT: f32 = 0.0;
    pub const WAY_OFF: f32 = -0.050;
    pub const MISS: f32 = -0.100;
    pub const HIT_MINE: f32 = -0.050;
    pub const HELD: f32 = 0.008;
    pub const LET_GO: f32 = -0.080;
}
