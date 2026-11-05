// SPDX-License-Identifier: GPL-3.0-or-later
//! Reusable headless-testable vector widget state models and painter helpers:
//! - [`RotaryKnobModel`]: Rotary dial with target arc and physical soft-takeover ghost ring.
//! - [`StepLedButtonModel`]: Step sequencer button and LED indicator for 303/controller hardware.
//! - [`EnvelopeCurveModel`]: ADSR curve visualization and polygon area generator.
//! - [`KeyOverlayBadgeModel`]: 25-key keyboard layout and AFX overlay badge manager.

use egui::{
    Align2, Color32, FontId, Painter, Pos2, Rect, Response, Rounding, Sense, Shape, Stroke, Ui,
    Vec2,
};
use serde::{Deserialize, Serialize};

/// Default sweep arc angle for rotary knobs in degrees (270 degrees total travel).
pub const DEFAULT_KNOB_SWEEP_DEGREES: f32 = 270.0;

/// Outcome of updating the physical controller position on a soft-takeover control.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SoftTakeoverOutcome {
    /// Physical control has caught the parameter: parameter updated to new value.
    Caught {
        /// The new normalized parameter value after catch.
        new_value: f32,
    },
    /// Physical control has not caught the parameter yet: distance remaining to target.
    PendingCatch {
        /// Absolute distance between physical position and target parameter value.
        distance: f32,
    },
    /// Physical control value was disconnected or cleared.
    Disconnected,
}

/// Headless vector rotary knob model with target arc and physical soft-takeover ghost ring.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RotaryKnobModel {
    /// Unique identifier for this knob.
    pub id: String,
    /// Human-readable parameter label (for example "Cutoff" or "Resonance").
    pub label: String,
    /// Engineering units (for example "Hz", "%", "ms", or "dB").
    pub unit: String,
    /// Current normalized logical parameter value in 0.0..=1.0.
    pub value: f32,
    /// Current physical hardware controller position in 0.0..=1.0, if connected.
    pub physical_value: Option<f32>,
    /// Whether the physical control has caught up with the patch target value.
    pub is_caught: bool,
    /// Pickup tolerance delta for soft-takeover catch (default 0.03).
    pub catch_threshold: f32,
    /// Whether the parameter is bipolar centered at 0.5 (for example pan, pitch, mod depth).
    pub bipolar: bool,
    /// Total travel sweep angle in degrees (default 270.0).
    pub sweep_degrees: f32,
}

impl Default for RotaryKnobModel {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            unit: String::new(),
            value: 0.5,
            physical_value: None,
            is_caught: true,
            catch_threshold: 0.03,
            bipolar: false,
            sweep_degrees: DEFAULT_KNOB_SWEEP_DEGREES,
        }
    }
}

impl RotaryKnobModel {
    /// Creates a new rotary knob model with default parameters.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>, value: f32) -> Self {
        let clamped = value.clamp(0.0, 1.0);
        Self {
            id: id.into(),
            label: label.into(),
            unit: String::new(),
            value: clamped,
            physical_value: None,
            is_caught: true,
            catch_threshold: 0.03,
            bipolar: false,
            sweep_degrees: DEFAULT_KNOB_SWEEP_DEGREES,
        }
    }

    /// Sets the engineering unit string.
    #[must_use]
    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = unit.into();
        self
    }

    /// Configures bipolar mode centered at 0.5.
    #[must_use]
    pub fn with_bipolar(mut self, bipolar: bool) -> Self {
        self.bipolar = bipolar;
        self
    }

    /// Sets the initial physical hardware controller position and soft-takeover status.
    #[must_use]
    pub fn with_physical_value(mut self, physical: f32) -> Self {
        let phys_clamped = physical.clamp(0.0, 1.0);
        self.physical_value = Some(phys_clamped);
        let delta = (phys_clamped - self.value).abs();
        self.is_caught = delta <= self.catch_threshold;
        self
    }

    /// Sets the pickup catch tolerance threshold.
    #[must_use]
    pub fn with_catch_threshold(mut self, threshold: f32) -> Self {
        self.catch_threshold = threshold.max(0.001);
        self
    }

    /// Updates the logical patch target value (for example from preset loading or automation).
    pub fn set_target_value(&mut self, new_target: f32) {
        self.value = new_target.clamp(0.0, 1.0);
        if let Some(phys) = self.physical_value {
            let delta = (phys - self.value).abs();
            if delta > self.catch_threshold {
                self.is_caught = false;
            }
        }
    }

    /// Updates the physical hardware controller position and executes soft-takeover arbitration.
    pub fn update_physical_value(&mut self, new_phys: f32) -> SoftTakeoverOutcome {
        let clamped = new_phys.clamp(0.0, 1.0);
        let prev = self.physical_value;
        self.physical_value = Some(clamped);

        if self.is_caught {
            self.value = clamped;
            return SoftTakeoverOutcome::Caught { new_value: clamped };
        }

        let delta = (clamped - self.value).abs();
        let within_threshold = delta <= self.catch_threshold;
        let crossed = match prev {
            Some(p) => (p - self.value) * (clamped - self.value) <= 0.0,
            None => false,
        };

        if within_threshold || crossed {
            self.is_caught = true;
            self.value = clamped;
            SoftTakeoverOutcome::Caught { new_value: clamped }
        } else {
            SoftTakeoverOutcome::PendingCatch { distance: delta }
        }
    }

    /// Computes the pointer angle in radians for any normalized position in 0.0..=1.0.
    #[must_use]
    pub fn angle_for_norm(&self, norm: f32) -> f32 {
        let sweep_rad = self.sweep_degrees.to_radians();
        (norm.clamp(0.0, 1.0) - 0.5) * sweep_rad
    }

    /// Returns the pointer angle in radians for the current logical target value.
    #[must_use]
    pub fn current_angle(&self) -> f32 {
        self.angle_for_norm(self.value)
    }

    /// Returns the pointer angle in radians for the physical hardware controller, if connected.
    #[must_use]
    pub fn physical_angle(&self) -> Option<f32> {
        self.physical_value.map(|phys| self.angle_for_norm(phys))
    }

    /// Computes a point on the dial given center coordinates, radius, and angle in radians.
    #[must_use]
    pub fn dial_point(center: Pos2, radius: f32, angle_rad: f32) -> Pos2 {
        Pos2::new(
            center.x + radius * angle_rad.sin(),
            center.y - radius * angle_rad.cos(),
        )
    }

    /// Returns the active arc start and end normalized positions.
    #[must_use]
    pub fn arc_norm_range(&self) -> (f32, f32) {
        if self.bipolar {
            (0.5, self.value)
        } else {
            (0.0, self.value)
        }
    }

    /// Generates polyline points for the complete background track.
    #[must_use]
    pub fn track_points(&self, center: Pos2, radius: f32, steps: usize) -> Vec<Pos2> {
        let num_steps = steps.max(8);
        (0..=num_steps)
            .map(|i| {
                let norm = i as f32 / num_steps as f32;
                let angle = self.angle_for_norm(norm);
                Self::dial_point(center, radius, angle)
            })
            .collect()
    }

    /// Generates polyline points for the active target parameter value arc.
    #[must_use]
    pub fn arc_points(&self, center: Pos2, radius: f32, steps: usize) -> Vec<Pos2> {
        let (start_norm, end_norm) = self.arc_norm_range();
        let total_steps = steps.max(8);
        let delta = (end_norm - start_norm).abs();
        let arc_steps = ((delta * total_steps as f32).ceil() as usize).max(2);

        (0..=arc_steps)
            .map(|i| {
                let frac = i as f32 / arc_steps as f32;
                let norm = start_norm + frac * (end_norm - start_norm);
                let angle = self.angle_for_norm(norm);
                Self::dial_point(center, radius, angle)
            })
            .collect()
    }

    /// Generates pointer line endpoints for the logical target value.
    #[must_use]
    pub fn pointer_endpoints(
        &self,
        center: Pos2,
        inner_radius: f32,
        outer_radius: f32,
    ) -> (Pos2, Pos2) {
        let angle = self.current_angle();
        (
            Self::dial_point(center, inner_radius, angle),
            Self::dial_point(center, outer_radius, angle),
        )
    }

    /// Generates pointer line endpoints for the physical soft-takeover ghost ring, if pending.
    #[must_use]
    pub fn ghost_pointer_endpoints(
        &self,
        center: Pos2,
        inner_radius: f32,
        outer_radius: f32,
    ) -> Option<(Pos2, Pos2)> {
        if self.is_caught {
            return None;
        }
        self.physical_value.map(|phys| {
            let angle = self.angle_for_norm(phys);
            (
                Self::dial_point(center, inner_radius, angle),
                Self::dial_point(center, outer_radius, angle),
            )
        })
    }

    /// Generates ghost ring arc points showing physical controller travel, if pending.
    #[must_use]
    pub fn ghost_ring_points(&self, center: Pos2, radius: f32, steps: usize) -> Option<Vec<Pos2>> {
        if self.is_caught {
            return None;
        }
        self.physical_value.map(|phys| {
            let total_steps = steps.max(8);
            let arc_steps = ((phys * total_steps as f32).ceil() as usize).max(2);
            (0..=arc_steps)
                .map(|i| {
                    let frac = i as f32 / arc_steps as f32;
                    let norm = frac * phys;
                    let angle = self.angle_for_norm(norm);
                    Self::dial_point(center, radius, angle)
                })
                .collect()
        })
    }

    /// Paints the complete vector rotary knob onto an egui Painter with crisp borders.
    pub fn paint(
        &self,
        painter: &Painter,
        rect: Rect,
        accent_color: Color32,
        ghost_color: Color32,
        bg_fill: Color32,
    ) {
        let size = rect.width().min(rect.height());
        let radius = size * 0.40;
        let center = Pos2::new(rect.center().x, rect.top() + radius + 4.0);

        // Knob body
        let body_radius = radius * 0.70;
        painter.circle_filled(center, body_radius, bg_fill);
        painter.circle_stroke(
            center,
            body_radius,
            Stroke::new(1.0, Color32::from_gray(100)),
        );

        // Background unlit track
        let track_radius = radius * 0.88;
        let track_width = (radius * 0.12).max(1.5);
        let track_pts = self.track_points(center, track_radius, 32);
        if track_pts.len() >= 2 {
            painter.add(Shape::line(
                track_pts,
                Stroke::new(track_width, Color32::from_gray(65)),
            ));
        }

        // Active parameter value arc
        let arc_pts = self.arc_points(center, track_radius, 32);
        if arc_pts.len() >= 2 {
            painter.add(Shape::line(
                arc_pts,
                Stroke::new(track_width, accent_color),
            ));
        }

        // Soft-takeover ghost ring and ghost pointer
        if !self.is_caught {
            let ghost_radius = radius * 0.98;
            if let Some(ghost_pts) = self.ghost_ring_points(center, ghost_radius, 32) {
                if ghost_pts.len() >= 2 {
                    painter.add(Shape::line(
                        ghost_pts,
                        Stroke::new(1.5, ghost_color),
                    ));
                }
            }
            if let Some((gp1, gp2)) =
                self.ghost_pointer_endpoints(center, body_radius * 0.35, body_radius * 0.90)
            {
                painter.line_segment([gp1, gp2], Stroke::new(2.0, ghost_color));
            }
        }

        // Logical parameter value pointer needle
        let (p1, p2) =
            self.pointer_endpoints(center, body_radius * 0.20, body_radius * 0.85);
        painter.line_segment([p1, p2], Stroke::new(2.0, Color32::WHITE));

        // Label and value text
        if !self.label.is_empty() {
            let label_pos = Pos2::new(rect.center().x, rect.bottom() - 14.0);
            painter.text(
                label_pos,
                Align2::CENTER_CENTER,
                &self.label,
                FontId::proportional(11.0),
                Color32::from_gray(210),
            );
        }
    }

    /// Renders the knob as an interactive egui widget with drag adjustments.
    pub fn show(&mut self, ui: &mut Ui, accent_color: Color32) -> Response {
        let desired_size = Vec2::new(64.0, 80.0);
        let (rect, mut response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());

        if response.dragged() {
            let drag_delta = -response.drag_delta().y * 0.005;
            let new_val = (self.value + drag_delta).clamp(0.0, 1.0);
            if (new_val - self.value).abs() > 0.0001 {
                self.value = new_val;
                response.mark_changed();
            }
        }

        if ui.is_rect_visible(rect) {
            let ghost_color = Color32::from_rgb(255, 176, 0); // Amber warning ghost
            let bg_fill = if response.hovered() {
                Color32::from_gray(45)
            } else {
                Color32::from_gray(30)
            };
            self.paint(ui.painter(), rect, accent_color, ghost_color, bg_fill);
        }

        response
    }
}

/// Illumination state of a step LED indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StepLedState {
    /// LED is extinguished.
    #[default]
    Off,
    /// LED is steadily illuminated.
    On,
    /// LED is flashing or pulsing.
    Blinking,
    /// LED is illuminated at low brightness for background status.
    Dim,
}

/// Headless vector step LED button model for 303 bassline sequencers and hardware controllers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepLedButtonModel {
    /// Zero-based step sequence index (for example 0..15 for a 16-step bar).
    pub step_index: usize,
    /// Human-facing step number (1..16).
    pub step_number: u8,
    /// Musical pitch note name in pitch mode (for example "C", "D#", "G").
    pub note_name: String,
    /// Octave transposition relative to base (-1, 0, +1).
    pub octave: i8,
    /// Gate trigger active for this step.
    pub gate_on: bool,
    /// 303 Accent flag active for this step.
    pub accent: bool,
    /// 303 Slide / portamento flag active for this step.
    pub slide: bool,
    /// Rest flag active for this step.
    pub rest: bool,
    /// Sequencer playhead currently executing this step.
    pub is_current_step: bool,
    /// Step selected for inspection or parameter editing in the GUI.
    pub is_selected: bool,
    /// Current LED illumination state.
    pub led_state: StepLedState,
}

impl StepLedButtonModel {
    /// Creates a new step button model with default initial state.
    #[must_use]
    pub fn new(step_index: usize) -> Self {
        Self {
            step_index,
            step_number: (step_index % 16 + 1) as u8,
            note_name: "C".to_string(),
            octave: 0,
            gate_on: false,
            accent: false,
            slide: false,
            rest: false,
            is_current_step: false,
            is_selected: false,
            led_state: StepLedState::Off,
        }
    }

    /// Sets the pitch note name and octave transpose.
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>, octave: i8) -> Self {
        self.note_name = note.into();
        self.octave = octave.clamp(-2, 2);
        self
    }

    /// Toggles gate trigger state and updates the LED illumination.
    pub fn toggle_gate(&mut self) -> bool {
        self.gate_on = !self.gate_on;
        self.update_led_state();
        self.gate_on
    }

    /// Toggles accent flag.
    pub fn toggle_accent(&mut self) -> bool {
        self.accent = !self.accent;
        self.accent
    }

    /// Toggles slide / portamento flag.
    pub fn toggle_slide(&mut self) -> bool {
        self.slide = !self.slide;
        self.slide
    }

    /// Toggles rest flag.
    pub fn toggle_rest(&mut self) -> bool {
        self.rest = !self.rest;
        if self.rest {
            self.gate_on = false;
        }
        self.update_led_state();
        self.rest
    }

    /// Sets whether the sequencer playhead is currently positioned on this step.
    pub fn set_current_step(&mut self, is_current: bool) {
        self.is_current_step = is_current;
        self.update_led_state();
    }

    /// Updates internal LED illumination state based on gate and playhead.
    pub fn update_led_state(&mut self) {
        if self.is_current_step {
            self.led_state = StepLedState::Blinking;
        } else if self.gate_on {
            self.led_state = StepLedState::On;
        } else if self.rest {
            self.led_state = StepLedState::Off;
        } else {
            self.led_state = StepLedState::Dim;
        }
    }

    /// Computes effective LED color based on theme, accent, and gate state.
    #[must_use]
    pub fn effective_led_color(
        &self,
        active_color: Color32,
        accent_color: Color32,
        off_color: Color32,
    ) -> Color32 {
        if self.is_current_step {
            Color32::WHITE
        } else if self.accent && self.gate_on {
            accent_color
        } else if self.gate_on {
            active_color
        } else if self.led_state == StepLedState::Dim {
            Color32::from_rgba_premultiplied(
                active_color.r() / 4,
                active_color.g() / 4,
                active_color.b() / 4,
                128,
            )
        } else {
            off_color
        }
    }

    /// Computes layout rectangles for a sequence of step buttons with spacing.
    #[must_use]
    pub fn step_rects(total_rect: Rect, count: usize, spacing: f32) -> Vec<Rect> {
        if count == 0 {
            return Vec::new();
        }
        let total_spacing = spacing * (count - 1) as f32;
        let button_width = ((total_rect.width() - total_spacing) / count as f32).max(1.0);
        let mut rects = Vec::with_capacity(count);

        for i in 0..count {
            let left = total_rect.left() + i as f32 * (button_width + spacing);
            rects.push(Rect::from_min_size(
                Pos2::new(left, total_rect.top()),
                Vec2::new(button_width, total_rect.height()),
            ));
        }

        rects
    }

    /// Paints the step button with crisp zero-rounding borders and LED indicators.
    pub fn paint(
        &self,
        painter: &Painter,
        rect: Rect,
        accent_color: Color32,
        is_hovered: bool,
    ) {
        let button_bg = if self.is_selected {
            Color32::from_gray(55)
        } else if is_hovered {
            Color32::from_gray(40)
        } else {
            Color32::from_gray(25)
        };

        // Crisp zero-rounding button rectangle
        painter.rect_filled(rect, Rounding::ZERO, button_bg);
        let border_stroke = if self.is_selected {
            Stroke::new(1.5, accent_color)
        } else if self.is_current_step {
            Stroke::new(1.5, Color32::WHITE)
        } else {
            Stroke::new(1.0, Color32::from_gray(70))
        };
        painter.rect_stroke(rect, Rounding::ZERO, border_stroke);

        // Top circular LED indicator
        let led_center = Pos2::new(rect.center().x, rect.top() + 8.0);
        let led_radius = 4.0;
        let led_col = self.effective_led_color(
            Color32::from_rgb(255, 48, 48), // Classic 303 red LED
            Color32::from_rgb(255, 180, 0),  // Accent amber
            Color32::from_gray(45),          // Off LED
        );
        painter.circle_filled(led_center, led_radius, led_col);
        painter.circle_stroke(
            led_center,
            led_radius,
            Stroke::new(0.5, Color32::from_gray(90)),
        );

        // Step number label
        let num_str = format!("{}", self.step_number);
        painter.text(
            Pos2::new(rect.center().x, rect.center().y - 2.0),
            Align2::CENTER_CENTER,
            num_str,
            FontId::proportional(11.0),
            if self.gate_on {
                Color32::WHITE
            } else {
                Color32::from_gray(160)
            },
        );

        // Slide and accent badges
        if self.slide {
            painter.text(
                Pos2::new(rect.right() - 6.0, rect.bottom() - 8.0),
                Align2::CENTER_CENTER,
                "~",
                FontId::monospace(10.0),
                accent_color,
            );
        }
        if self.accent {
            painter.text(
                Pos2::new(rect.left() + 6.0, rect.bottom() - 8.0),
                Align2::CENTER_CENTER,
                "A",
                FontId::proportional(9.0),
                Color32::from_rgb(255, 180, 0),
            );
        }
    }

    /// Renders the step button as an interactive egui widget.
    pub fn show(&mut self, ui: &mut Ui, accent_color: Color32) -> Response {
        let desired_size = Vec2::new(32.0, 48.0);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

        if response.clicked() {
            self.toggle_gate();
        }

        if ui.is_rect_visible(rect) {
            self.paint(ui.painter(), rect, accent_color, response.hovered());
        }

        response
    }
}

/// Headless vector ADSR envelope curve model for visualization and polygon area fill.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvelopeCurveModel {
    /// Normalized attack duration in 0.0..=1.0.
    pub attack: f32,
    /// Normalized decay duration in 0.0..=1.0.
    pub decay: f32,
    /// Normalized sustain amplitude level in 0.0..=1.0.
    pub sustain: f32,
    /// Normalized release duration in 0.0..=1.0.
    pub release: f32,
    /// Curvature coefficient for attack slope (1.0 = linear, >1.0 exponential).
    pub attack_curvature: f32,
    /// Curvature coefficient for decay slope (1.0 = linear, >1.0 exponential).
    pub decay_curvature: f32,
    /// Curvature coefficient for release slope (1.0 = linear, >1.0 exponential).
    pub release_curvature: f32,
    /// Normalized gate hold duration between decay and release (default 0.20).
    pub hold_duration: f32,
}

impl Default for EnvelopeCurveModel {
    fn default() -> Self {
        Self {
            attack: 0.15,
            decay: 0.25,
            sustain: 0.60,
            release: 0.30,
            attack_curvature: 1.0,
            decay_curvature: 1.0,
            release_curvature: 1.0,
            hold_duration: 0.20,
        }
    }
}

impl EnvelopeCurveModel {
    /// Creates a new ADSR envelope model.
    #[must_use]
    pub fn new(attack: f32, decay: f32, sustain: f32, release: f32) -> Self {
        Self {
            attack: attack.clamp(0.0, 1.0),
            decay: decay.clamp(0.0, 1.0),
            sustain: sustain.clamp(0.0, 1.0),
            release: release.clamp(0.0, 1.0),
            attack_curvature: 1.0,
            decay_curvature: 1.0,
            release_curvature: 1.0,
            hold_duration: 0.20,
        }
    }

    /// Configures custom exponential or logarithmic curvatures for each stage.
    #[must_use]
    pub fn with_curvatures(
        mut self,
        attack_curv: f32,
        decay_curv: f32,
        release_curv: f32,
    ) -> Self {
        self.attack_curvature = attack_curv.max(0.1);
        self.decay_curvature = decay_curv.max(0.1);
        self.release_curvature = release_curv.max(0.1);
        self
    }

    /// Sets the gate hold duration.
    #[must_use]
    pub fn with_hold(mut self, hold: f32) -> Self {
        self.hold_duration = hold.clamp(0.05, 0.50);
        self
    }

    /// Computes proportional duration fractions for each stage summing to 1.0.
    #[must_use]
    pub fn stage_fractions(&self) -> (f32, f32, f32, f32) {
        let a = self.attack.max(0.02);
        let d = self.decay.max(0.02);
        let h = self.hold_duration.max(0.02);
        let r = self.release.max(0.02);
        let total = a + d + h + r;
        (a / total, d / total, h / total, r / total)
    }

    /// Evaluates envelope amplitude in 0.0..=1.0 at normalized time in 0.0..=1.0.
    #[must_use]
    pub fn evaluate_at(&self, t_norm: f32) -> f32 {
        let t = t_norm.clamp(0.0, 1.0);
        let (fa, fd, fh, fr) = self.stage_fractions();

        let t_attack_end = fa;
        let t_decay_end = fa + fd;
        let t_hold_end = fa + fd + fh;

        if t <= t_attack_end {
            let stage_t = (t / fa).clamp(0.0, 1.0);
            if (self.attack_curvature - 1.0).abs() < 0.001 {
                stage_t
            } else {
                stage_t.powf(1.0 / self.attack_curvature)
            }
        } else if t <= t_decay_end {
            let stage_t = ((t - t_attack_end) / fd).clamp(0.0, 1.0);
            let drop = if (self.decay_curvature - 1.0).abs() < 0.001 {
                stage_t
            } else {
                stage_t.powf(self.decay_curvature)
            };
            1.0 - (1.0 - self.sustain) * drop
        } else if t <= t_hold_end {
            self.sustain
        } else {
            let stage_t = ((t - t_hold_end) / fr).clamp(0.0, 1.0);
            let drop = if (self.release_curvature - 1.0).abs() < 0.001 {
                stage_t
            } else {
                stage_t.powf(self.release_curvature)
            };
            (self.sustain * (1.0 - drop)).max(0.0)
        }
    }

    /// Computes the five key landmark vertices for the envelope inside a bounding box:
    /// (1) Origin, (2) Attack peak, (3) Sustain start, (4) Sustain end, (5) Release finish.
    #[must_use]
    pub fn key_vertices(&self, rect: Rect) -> [Pos2; 5] {
        let (fa, fd, fh, _fr) = self.stage_fractions();
        let w = rect.width();
        let h = rect.height();

        let p0 = Pos2::new(rect.left(), rect.bottom());
        let p1 = Pos2::new(rect.left() + fa * w, rect.top());
        let p2 = Pos2::new(
            rect.left() + (fa + fd) * w,
            rect.bottom() - self.sustain * h,
        );
        let p3 = Pos2::new(
            rect.left() + (fa + fd + fh) * w,
            rect.bottom() - self.sustain * h,
        );
        let p4 = Pos2::new(rect.right(), rect.bottom());

        [p0, p1, p2, p3, p4]
    }

    /// Generates polyline vertex points across the full envelope curve.
    #[must_use]
    pub fn generate_polyline(&self, rect: Rect, samples: usize) -> Vec<Pos2> {
        let num_samples = samples.max(16);
        let w = rect.width();
        let h = rect.height();

        (0..=num_samples)
            .map(|i| {
                let t = i as f32 / num_samples as f32;
                let amp = self.evaluate_at(t);
                Pos2::new(rect.left() + t * w, rect.bottom() - amp * h)
            })
            .collect()
    }

    /// Generates closed polygon vertices for translucent filled area under the curve.
    #[must_use]
    pub fn generate_filled_polygon(&self, rect: Rect, samples: usize) -> Vec<Pos2> {
        let mut pts = self.generate_polyline(rect, samples);
        pts.push(Pos2::new(rect.right(), rect.bottom()));
        pts.push(Pos2::new(rect.left(), rect.bottom()));
        pts
    }

    /// Paints the complete envelope curve and filled background polygon.
    pub fn paint(
        &self,
        painter: &Painter,
        rect: Rect,
        stroke: Stroke,
        fill_color: Option<Color32>,
        handle_color: Option<Color32>,
    ) {
        // Zero-rounding crisp border frame
        painter.rect_stroke(
            rect,
            Rounding::ZERO,
            Stroke::new(1.0, Color32::from_gray(60)),
        );

        // Filled area under curve
        if let Some(fill) = fill_color {
            let poly_pts = self.generate_filled_polygon(rect, 48);
            if poly_pts.len() >= 3 {
                painter.add(Shape::convex_polygon(
                    poly_pts,
                    fill,
                    Stroke::NONE,
                ));
            }
        }

        // Polyline curve stroke
        let line_pts = self.generate_polyline(rect, 48);
        if line_pts.len() >= 2 {
            painter.add(Shape::line(line_pts, stroke));
        }

        // Circular handles at key landmark vertices
        if let Some(hcol) = handle_color {
            let landmarks = self.key_vertices(rect);
            for pt in landmarks {
                painter.circle_filled(pt, 3.0, hcol);
                painter.circle_stroke(
                    pt,
                    3.0,
                    Stroke::new(0.5, Color32::BLACK),
                );
            }
        }
    }

    /// Renders the envelope curve as an egui visualization widget.
    pub fn show(&self, ui: &mut Ui, stroke_color: Color32) -> Response {
        let desired_size = Vec2::new(160.0, 80.0);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::hover());

        if ui.is_rect_visible(rect) {
            let fill_color = Color32::from_rgba_premultiplied(
                stroke_color.r() / 4,
                stroke_color.g() / 4,
                stroke_color.b() / 4,
                80,
            );
            self.paint(
                ui.painter(),
                rect,
                Stroke::new(2.0, stroke_color),
                Some(fill_color),
                Some(Color32::WHITE),
            );
        }

        response
    }
}

/// Headless vector keyboard model for the 25 keys on the Novation Bass Station II,
/// supporting AFX Mode per-key patch overlays and secondary faceplate function labels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyOverlayBadgeModel {
    /// Zero-based key index (0..24 for 25 keys).
    pub key_index: u8,
    /// MIDI note number (36..60 for C2 to C4).
    pub midi_note: u8,
    /// Note name (for example "C", "C#", "D").
    pub note_name: String,
    /// Whether this is a black accidental piano key.
    pub is_black_key: bool,
    /// Optional AFX Mode patch slot ID assigned to this key.
    pub afx_patch_id: Option<u16>,
    /// Optional AFX Mode patch name assigned to this key.
    pub afx_patch_name: Option<String>,
    /// Primary secondary faceplate function label on Bass Station II.
    pub function_label: Option<String>,
    /// Sub-parameter secondary label on Bass Station II.
    pub sub_function_label: Option<String>,
    /// Whether this key is currently held down or triggered.
    pub is_pressed: bool,
    /// Category color override in hex for AFX Mode badges (for example "#00D2FF").
    pub badge_color_hex: Option<String>,
}

impl KeyOverlayBadgeModel {
    /// Creates a key overlay badge model for a given key index in 0..24.
    #[must_use]
    pub fn new(key_index: u8) -> Self {
        let idx = key_index.min(24);
        let is_black = Self::is_black_key_index(idx);
        let note = Self::note_name_for_index(idx);
        let (func, sub) = Self::bs2_function_for_key(idx);

        Self {
            key_index: idx,
            midi_note: 36 + idx,
            note_name: note.to_string(),
            is_black_key: is_black,
            afx_patch_id: None,
            afx_patch_name: None,
            function_label: Some(func.to_string()),
            sub_function_label: Some(sub.to_string()),
            is_pressed: false,
            badge_color_hex: None,
        }
    }

    /// Determines whether a 0..24 key index represents a black accidental key.
    #[must_use]
    pub const fn is_black_key_index(key_index: u8) -> bool {
        let note_in_octave = (key_index % 12) as usize;
        matches!(note_in_octave, 1 | 3 | 6 | 8 | 10)
    }

    /// Returns the note name string for a 0..24 key index.
    #[must_use]
    pub const fn note_name_for_index(key_index: u8) -> &'static str {
        const NOTE_NAMES: [&str; 12] = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        NOTE_NAMES[(key_index % 12) as usize]
    }

    /// Returns Novation Bass Station II printed faceplate functions for each key.
    #[must_use]
    pub const fn bs2_function_for_key(key_index: u8) -> (&'static str, &'static str) {
        match key_index {
            0 => ("OSC 1", "WAVE / OCT"),
            1 => ("OSC 1", "RANGE"),
            2 => ("OSC 2", "WAVE"),
            3 => ("OSC 2", "RANGE"),
            4 => ("SUB OSC", "WAVE"),
            5 => ("SUB OSC", "OCTAVE"),
            6 => ("FILTER", "TYPE"),
            7 => ("FILTER", "SLOPE"),
            8 => ("FILTER", "SHAPE"),
            9 => ("LFO 1", "WAVE"),
            10 => ("LFO 1", "SPEED"),
            11 => ("LFO 2", "WAVE"),
            12 => ("LFO 2", "SPEED"),
            13 => ("MOD ENV", "ATTACK"),
            14 => ("MOD ENV", "DECAY"),
            15 => ("MOD ENV", "SUSTAIN"),
            16 => ("MOD ENV", "RELEASE"),
            17 => ("AMP ENV", "ATTACK"),
            18 => ("AMP ENV", "DECAY"),
            19 => ("AMP ENV", "SUSTAIN"),
            20 => ("AMP ENV", "RELEASE"),
            21 => ("ARP", "MODE"),
            22 => ("ARP", "RHYTHM"),
            23 => ("ARP", "OCTAVE"),
            24 => ("AFX / PARA", "OVERLAY"),
            _ => ("UNKNOWN", ""),
        }
    }

    /// Creates the complete canonical set of 25 keys for the Bass Station II.
    #[must_use]
    pub fn bs2_25_keys() -> Vec<Self> {
        (0..25).map(Self::new).collect()
    }

    /// Assigns an AFX Mode patch overlay to this key.
    #[must_use]
    pub fn with_afx_patch(
        mut self,
        patch_id: u16,
        name: impl Into<String>,
        color_hex: impl Into<String>,
    ) -> Self {
        self.afx_patch_id = Some(patch_id);
        self.afx_patch_name = Some(name.into());
        self.badge_color_hex = Some(color_hex.into());
        self
    }

    /// Clears any assigned AFX Mode patch overlay.
    pub fn clear_afx_patch(&mut self) {
        self.afx_patch_id = None;
        self.afx_patch_name = None;
        self.badge_color_hex = None;
    }

    /// Headless calculation of piano key rectangles for all 25 keys:
    /// Returns a tuple of (15 white key rectangles, 10 black key rectangles).
    #[must_use]
    pub fn calculate_25_key_layout(total_rect: Rect) -> (Vec<Rect>, Vec<Rect>) {
        const WHITE_KEY_COUNT: usize = 15;
        let white_width = total_rect.width() / WHITE_KEY_COUNT as f32;
        let black_width = white_width * 0.60;
        let black_height = total_rect.height() * 0.62;

        let mut white_rects = Vec::with_capacity(WHITE_KEY_COUNT);
        for i in 0..WHITE_KEY_COUNT {
            let left = total_rect.left() + i as f32 * white_width;
            white_rects.push(Rect::from_min_size(
                Pos2::new(left, total_rect.top()),
                Vec2::new(white_width, total_rect.height()),
            ));
        }

        // Octave 1 black keys: C# (boundary 1), D# (boundary 2), F# (boundary 4), G# (boundary 5), A# (boundary 6)
        // Octave 2 black keys: C# (boundary 8), D# (boundary 9), F# (boundary 11), G# (boundary 12), A# (boundary 13)
        let black_boundaries = [1, 2, 4, 5, 6, 8, 9, 11, 12, 13];
        let mut black_rects = Vec::with_capacity(black_boundaries.len());
        for b in black_boundaries {
            let center_x = total_rect.left() + b as f32 * white_width;
            black_rects.push(Rect::from_min_size(
                Pos2::new(center_x - black_width * 0.5, total_rect.top()),
                Vec2::new(black_width, black_height),
            ));
        }

        (white_rects, black_rects)
    }

    /// Paints the key rectangle and top badge with crisp zero-rounding borders.
    pub fn paint_key_and_badge(
        &self,
        painter: &Painter,
        key_rect: Rect,
        badge_rect: Rect,
        accent_color: Color32,
    ) {
        // 1. Piano key surface
        let fill = if self.is_pressed {
            accent_color
        } else if self.is_black_key {
            Color32::from_gray(25)
        } else {
            Color32::from_gray(240)
        };

        painter.rect_filled(key_rect, Rounding::ZERO, fill);
        painter.rect_stroke(
            key_rect,
            Rounding::ZERO,
            Stroke::new(1.0, Color32::from_gray(80)),
        );

        // 2. AFX / function badge above the key
        let badge_fill = if let Some(ref hex) = self.badge_color_hex {
            crate::parse_hex_color(hex).unwrap_or(Color32::from_gray(50))
        } else if self.is_pressed {
            accent_color
        } else {
            Color32::from_gray(35)
        };

        painter.rect_filled(badge_rect, Rounding::ZERO, badge_fill);
        painter.rect_stroke(
            badge_rect,
            Rounding::ZERO,
            Stroke::new(1.0, Color32::from_gray(70)),
        );

        // Badge text: display AFX patch name if assigned, else BS2 faceplate label
        let display_text = if let Some(ref afx_name) = self.afx_patch_name {
            afx_name.as_str()
        } else if let Some(ref func) = self.function_label {
            func.as_str()
        } else {
            self.note_name.as_str()
        };

        painter.text(
            badge_rect.center(),
            Align2::CENTER_CENTER,
            display_text,
            FontId::proportional(9.0),
            Color32::WHITE,
        );
    }
}

// =============================================================================
// Percussion Vector Widgets (Nord Drum 2 & Konduktiva)
// =============================================================================

/// Headless vector drum pad model with dynamic velocity strike response, peak hold,
/// choke grouping, and Nord Drum 2 voice channel assignment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrumPadVelocityModel {
    /// Zero-based channel index (0..5 for Nord Drum 2 Voices 1..6).
    pub channel_index: usize,
    /// Channel display name (for example "Kick", "Snare", "Tom Lo", "Voice 1").
    pub label: String,
    /// MIDI trigger note number (default 60 + channel_index).
    pub midi_note: u8,
    /// Current velocity (0..127).
    pub velocity: u8,
    /// Peak hold velocity (0..127).
    pub peak_velocity: u8,
    /// Whether the pad is currently struck or active.
    pub is_triggered: bool,
    /// Whether this channel is currently focused for parameter editing (CC 70).
    pub is_focused: bool,
    /// Whether this channel is muted.
    pub is_muted: bool,
    /// Optional choke group assignment (for example 1 for open/closed hi-hat).
    pub choke_group: Option<u8>,
}

impl DrumPadVelocityModel {
    /// Creates a new drum pad velocity model for a voice channel index.
    #[must_use]
    pub fn new(channel_index: usize, label: impl Into<String>) -> Self {
        let ch = channel_index.min(5);
        Self {
            channel_index: ch,
            label: label.into(),
            midi_note: 60 + (ch as u8),
            velocity: 0,
            peak_velocity: 0,
            is_triggered: false,
            is_focused: false,
            is_muted: false,
            choke_group: None,
        }
    }

    /// Sets the MIDI trigger note number.
    #[must_use]
    pub fn with_midi_note(mut self, note: u8) -> Self {
        self.midi_note = note;
        self
    }

    /// Sets the current velocity.
    #[must_use]
    pub fn with_velocity(mut self, velocity: u8) -> Self {
        let v = velocity.min(127);
        self.velocity = v;
        self.peak_velocity = self.peak_velocity.max(v);
        self
    }

    /// Sets the choke group.
    #[must_use]
    pub fn with_choke_group(mut self, choke: u8) -> Self {
        self.choke_group = Some(choke);
        self
    }

    /// Sets editor focus state.
    #[must_use]
    pub fn with_focused(mut self, focused: bool) -> Self {
        self.is_focused = focused;
        self
    }

    /// Sets mute state.
    #[must_use]
    pub fn with_muted(mut self, muted: bool) -> Self {
        self.is_muted = muted;
        self
    }

    /// Triggers the pad with a strike velocity in 0..127.
    pub fn trigger(&mut self, velocity: u8) {
        let v = velocity.min(127);
        self.velocity = v;
        self.peak_velocity = self.peak_velocity.max(v);
        self.is_triggered = true;
    }

    /// Releases the active trigger state.
    pub fn release(&mut self) {
        self.is_triggered = false;
    }

    /// Decays the peak hold velocity by a step decrement.
    pub fn decay_peak(&mut self, step: u8) {
        self.peak_velocity = self.peak_velocity.saturating_sub(step);
    }

    /// Returns normalized velocity in 0.0..=1.0.
    #[must_use]
    pub fn normalized_velocity(&self) -> f32 {
        self.velocity as f32 / 127.0
    }

    /// Returns normalized peak hold velocity in 0.0..=1.0.
    #[must_use]
    pub fn normalized_peak(&self) -> f32 {
        self.peak_velocity as f32 / 127.0
    }

    /// Returns the 1-based voice number (1..6).
    #[must_use]
    pub fn voice_number(&self) -> usize {
        self.channel_index + 1
    }

    /// Computes layout rectangles for a grid of pads (for example 6 pads in 2 rows x 3 columns).
    #[must_use]
    pub fn compute_pad_rects(total_rect: Rect, count: usize, cols: usize, spacing: f32) -> Vec<Rect> {
        if count == 0 || cols == 0 {
            return Vec::new();
        }
        let rows = count.div_ceil(cols);
        let pad_width = ((total_rect.width() - spacing * (cols - 1) as f32) / cols as f32).max(1.0);
        let pad_height = ((total_rect.height() - spacing * (rows - 1) as f32) / rows as f32).max(1.0);

        let mut rects = Vec::with_capacity(count);
        for idx in 0..count {
            let col = idx % cols;
            let row = idx / cols;
            let min_x = total_rect.left() + col as f32 * (pad_width + spacing);
            let min_y = total_rect.top() + row as f32 * (pad_height + spacing);
            rects.push(Rect::from_min_size(
                Pos2::new(min_x, min_y),
                Vec2::new(pad_width, pad_height),
            ));
        }
        rects
    }

    /// Computes the bounding rect for the interior horizontal velocity meter.
    #[must_use]
    pub fn velocity_bar_rect(&self, pad_rect: Rect) -> Rect {
        let pad_h = pad_rect.height();
        let bar_h = (pad_h * 0.12).clamp(4.0, 10.0);
        let margin_x = 4.0;
        let margin_b = 4.0;
        Rect::from_min_size(
            Pos2::new(pad_rect.left() + margin_x, pad_rect.bottom() - bar_h - margin_b),
            Vec2::new((pad_rect.width() - margin_x * 2.0).max(1.0), bar_h),
        )
    }

    /// Computes the active filled portion of the velocity meter.
    #[must_use]
    pub fn velocity_fill_rect(&self, pad_rect: Rect) -> Rect {
        let bar = self.velocity_bar_rect(pad_rect);
        let fill_w = bar.width() * self.normalized_velocity();
        Rect::from_min_size(bar.min, Vec2::new(fill_w, bar.height()))
    }

    /// Paints the drum pad velocity widget.
    pub fn paint(
        &self,
        painter: &Painter,
        rect: Rect,
        accent_color: Color32,
        readout_color: Color32,
    ) {
        // Pad strike surface fill
        let bg_fill = if self.is_muted {
            Color32::from_gray(18)
        } else if self.is_triggered {
            Color32::from_rgba_premultiplied(
                (accent_color.r() as u16 * 140 / 255) as u8,
                (accent_color.g() as u16 * 140 / 255) as u8,
                (accent_color.b() as u16 * 140 / 255) as u8,
                220,
            )
        } else {
            Color32::from_gray(30)
        };

        let stroke_color = if self.is_focused {
            accent_color
        } else if self.is_triggered {
            Color32::WHITE
        } else {
            Color32::from_gray(65)
        };
        let stroke_width = if self.is_focused { 2.0 } else { 1.0 };

        painter.rect_filled(rect, Rounding::ZERO, bg_fill);
        painter.rect_stroke(rect, Rounding::ZERO, Stroke::new(stroke_width, stroke_color));

        // Header: Channel / voice number and optional choke badge
        let header_text = format!("CH {}", self.voice_number());
        painter.text(
            Pos2::new(rect.left() + 6.0, rect.top() + 6.0),
            Align2::LEFT_TOP,
            &header_text,
            FontId::proportional(11.0),
            if self.is_focused { readout_color } else { Color32::from_gray(180) },
        );

        if let Some(choke) = self.choke_group {
            let choke_text = format!("EX{}", choke);
            painter.text(
                Pos2::new(rect.right() - 6.0, rect.top() + 6.0),
                Align2::RIGHT_TOP,
                &choke_text,
                FontId::proportional(9.0),
                Color32::from_rgb(255, 120, 60),
            );
        }

        // Center: Instrument label (e.g. "KICK")
        let label_color = if self.is_muted {
            Color32::from_gray(80)
        } else if self.is_triggered {
            Color32::WHITE
        } else {
            Color32::from_gray(230)
        };
        painter.text(
            Pos2::new(rect.center().x, rect.center().y - 4.0),
            Align2::CENTER_CENTER,
            &self.label,
            FontId::proportional(14.0),
            label_color,
        );

        // Subtitle: MIDI Note number
        let note_text = format!("#{}", self.midi_note);
        painter.text(
            Pos2::new(rect.left() + 6.0, rect.center().y + 12.0),
            Align2::LEFT_TOP,
            &note_text,
            FontId::proportional(9.0),
            Color32::from_gray(140),
        );

        // Velocity readout in high-contrast amber LED readout color
        let vel_text = if self.velocity > 0 {
            format!("V:{:3}", self.velocity)
        } else {
            "IDLE".to_string()
        };
        painter.text(
            Pos2::new(rect.right() - 6.0, rect.center().y + 12.0),
            Align2::RIGHT_TOP,
            &vel_text,
            FontId::monospace(9.0),
            readout_color,
        );

        // Velocity Meter Bar at bottom
        let bar_slot = self.velocity_bar_rect(rect);
        painter.rect_filled(bar_slot, Rounding::ZERO, Color32::from_gray(18));
        painter.rect_stroke(bar_slot, Rounding::ZERO, Stroke::new(0.5, Color32::from_gray(50)));

        if self.velocity > 0 {
            let fill_rect = self.velocity_fill_rect(rect);
            painter.rect_filled(fill_rect, Rounding::ZERO, accent_color);
        }

        // Peak hold tick marker
        if self.peak_velocity > 0 {
            let peak_x = bar_slot.left() + bar_slot.width() * self.normalized_peak();
            painter.line_segment(
                [
                    Pos2::new(peak_x, bar_slot.top()),
                    Pos2::new(peak_x, bar_slot.bottom()),
                ],
                Stroke::new(1.5, readout_color),
            );
        }
    }

    /// Renders the interactive drum pad velocity widget.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        accent_color: Color32,
        readout_color: Color32,
    ) -> Response {
        let desired_size = Vec2::new(96.0, 72.0);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());

        if response.clicked() {
            self.trigger(100);
        }

        if ui.is_rect_visible(rect) {
            self.paint(ui.painter(), rect, accent_color, readout_color);
        }

        response
    }
}

/// Standalone percussion vector widget for rendering a drum pad velocity strike indicator.
pub fn drum_pad_velocity_widget(
    ui: &mut Ui,
    model: &mut DrumPadVelocityModel,
    accent_color: Color32,
    readout_color: Color32,
) -> Response {
    model.show(ui, accent_color, readout_color)
}

/// Headless harmonic spectrum resonator model for Nord Drum 2 Tone synthesis models,
/// representing harmonic partial amplitudes, resonance Q, and spectral decay tilt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResonantSpectrumBarModel {
    /// Number of harmonic partial bars displayed (typically 8 to 16).
    pub partial_count: usize,
    /// Base fundamental resonant frequency in Hz.
    pub fundamental_hz: f32,
    /// Normalized amplitude weights for each harmonic partial in 0.0..=1.0.
    pub harmonic_weights: Vec<f32>,
    /// Resonance Q / sharpness factor in 0.0..=1.0.
    pub resonance_q: f32,
    /// Bipolar spectral tilt in -1.0..=1.0 (negative = dark/low-pass, positive = bright/high-pass).
    pub spectral_tilt: f32,
    /// High-frequency decay damping coefficient in 0.0..=1.0.
    pub decay_damping: f32,
}

impl ResonantSpectrumBarModel {
    /// Creates a new resonant spectrum bar model with a specified number of partial bars.
    #[must_use]
    pub fn new(partial_count: usize) -> Self {
        let count = partial_count.clamp(1, 32);
        let mut model = Self {
            partial_count: count,
            fundamental_hz: 220.0,
            harmonic_weights: vec![0.0; count],
            resonance_q: 0.70,
            spectral_tilt: 0.0,
            decay_damping: 0.30,
        };
        model.recompute_harmonics();
        model
    }

    /// Creates a resonant spectrum model configured for standard Nord Drum 2 acoustic presets.
    #[must_use]
    pub fn from_preset(preset_name: &str) -> Self {
        let count = 8;
        let mut model = Self::new(count);
        match preset_name.trim().to_lowercase().as_str() {
            "bell" | "chime" => {
                model.fundamental_hz = 520.0;
                model.resonance_q = 0.90;
                model.spectral_tilt = 0.35;
                model.decay_damping = 0.15;
                model.harmonic_weights = vec![1.0, 0.72, 0.45, 0.85, 0.30, 0.60, 0.20, 0.40];
            }
            "membrane" | "drumhead" => {
                model.fundamental_hz = 140.0;
                model.resonance_q = 0.60;
                model.spectral_tilt = -0.40;
                model.decay_damping = 0.50;
                model.harmonic_weights = vec![1.0, 0.62, 0.38, 0.24, 0.15, 0.09, 0.05, 0.02];
            }
            "cymbal" | "metal" => {
                model.fundamental_hz = 880.0;
                model.resonance_q = 0.85;
                model.spectral_tilt = 0.50;
                model.decay_damping = 0.10;
                model.harmonic_weights = vec![0.40, 0.65, 0.90, 0.75, 1.0, 0.85, 0.70, 0.55];
            }
            "sub" | "kick" => {
                model.fundamental_hz = 55.0;
                model.resonance_q = 0.40;
                model.spectral_tilt = -0.80;
                model.decay_damping = 0.75;
                model.harmonic_weights = vec![1.0, 0.30, 0.08, 0.02, 0.0, 0.0, 0.0, 0.0];
            }
            _ => {
                model.recompute_harmonics();
            }
        }
        model
    }

    /// Sets explicit harmonic partial weights.
    #[must_use]
    pub fn with_harmonic_weights(mut self, weights: Vec<f32>) -> Self {
        self.partial_count = weights.len().clamp(1, 32);
        self.harmonic_weights = weights.into_iter().map(|w| w.clamp(0.0, 1.0)).collect();
        self
    }

    /// Sets the fundamental resonance frequency in Hz.
    #[must_use]
    pub fn with_fundamental(mut self, hz: f32) -> Self {
        self.fundamental_hz = hz.max(10.0);
        self
    }

    /// Sets the resonance Q factor in 0.0..=1.0.
    #[must_use]
    pub fn with_resonance_q(mut self, q: f32) -> Self {
        self.resonance_q = q.clamp(0.0, 1.0);
        self
    }

    /// Sets the spectral tilt in -1.0..=1.0.
    #[must_use]
    pub fn with_spectral_tilt(mut self, tilt: f32) -> Self {
        self.spectral_tilt = tilt.clamp(-1.0, 1.0);
        self
    }

    /// Sets the high-frequency decay damping in 0.0..=1.0.
    #[must_use]
    pub fn with_decay_damping(mut self, damping: f32) -> Self {
        self.decay_damping = damping.clamp(0.0, 1.0);
        self
    }

    /// Sets individual partial weight by zero-based index.
    pub fn set_partial_weight(&mut self, index: usize, weight: f32) {
        if index < self.harmonic_weights.len() {
            self.harmonic_weights[index] = weight.clamp(0.0, 1.0);
        }
    }

    /// Recomputes harmonic weights from fundamental, resonance Q, tilt, and damping.
    pub fn recompute_harmonics(&mut self) {
        self.harmonic_weights.resize(self.partial_count, 0.0);
        for i in 0..self.partial_count {
            let harmonic_idx = (i + 1) as f32;
            let natural = 1.0 / harmonic_idx.sqrt();
            let tilt_factor = 1.0 + self.spectral_tilt * ((i as f32) / self.partial_count as f32 - 0.5) * 2.0;
            let damp = (1.0 - self.decay_damping * 0.15).powf(harmonic_idx);
            let q_boost = if i == 0 {
                1.0 + self.resonance_q * 0.5
            } else if i == 1 {
                1.0 + self.resonance_q * 0.25
            } else {
                1.0
            };

            let weight = (natural * tilt_factor * damp * q_boost).clamp(0.02, 1.0);
            self.harmonic_weights[i] = weight;
        }
    }

    /// Computes layout bounding boxes for all partial bar slots across total_rect.
    #[must_use]
    pub fn bar_rects(&self, total_rect: Rect, spacing: f32) -> Vec<Rect> {
        let count = self.partial_count;
        if count == 0 {
            return Vec::new();
        }
        let bar_width = ((total_rect.width() - spacing * (count - 1) as f32) / count as f32).max(1.0);
        let mut rects = Vec::with_capacity(count);
        for i in 0..count {
            let left = total_rect.left() + i as f32 * (bar_width + spacing);
            rects.push(Rect::from_min_size(
                Pos2::new(left, total_rect.top()),
                Vec2::new(bar_width, total_rect.height()),
            ));
        }
        rects
    }

    /// Computes the active filled portion of each partial bar based on its weight.
    #[must_use]
    pub fn filled_bar_rects(&self, total_rect: Rect, spacing: f32) -> Vec<Rect> {
        let slots = self.bar_rects(total_rect, spacing);
        slots
            .into_iter()
            .enumerate()
            .map(|(i, slot)| {
                let weight = self.harmonic_weights.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let fill_height = (slot.height() * weight).max(1.0);
                Rect::from_min_size(
                    Pos2::new(slot.left(), slot.bottom() - fill_height),
                    Vec2::new(slot.width(), fill_height),
                )
            })
            .collect()
    }

    /// Generates polyline vertices connecting the top centers of the harmonic bars.
    #[must_use]
    pub fn envelope_polyline(&self, total_rect: Rect, spacing: f32) -> Vec<Pos2> {
        let filled = self.filled_bar_rects(total_rect, spacing);
        filled
            .into_iter()
            .map(|bar| Pos2::new(bar.center().x, bar.top()))
            .collect()
    }

    /// Paints the resonant spectrum bar widget.
    pub fn paint(
        &self,
        painter: &Painter,
        rect: Rect,
        accent_color: Color32,
        readout_color: Color32,
    ) {
        // Frame background
        painter.rect_filled(rect, Rounding::ZERO, Color32::from_gray(20));
        painter.rect_stroke(rect, Rounding::ZERO, Stroke::new(1.0, Color32::from_gray(50)));

        // Reference amplitude horizontal grid lines (-6dB, -12dB)
        let mid_y = rect.top() + rect.height() * 0.5;
        let qtr_y = rect.top() + rect.height() * 0.25;
        painter.line_segment(
            [Pos2::new(rect.left(), mid_y), Pos2::new(rect.right(), mid_y)],
            Stroke::new(0.5, Color32::from_gray(40)),
        );
        painter.line_segment(
            [Pos2::new(rect.left(), qtr_y), Pos2::new(rect.right(), qtr_y)],
            Stroke::new(0.5, Color32::from_gray(35)),
        );

        let spacing = 3.0;
        let slots = self.bar_rects(rect, spacing);
        let filled = self.filled_bar_rects(rect, spacing);

        for (i, slot) in slots.iter().enumerate() {
            // Unfilled slot track
            painter.rect_filled(*slot, Rounding::ZERO, Color32::from_gray(30));

            // Filled partial amplitude
            if let Some(fill) = filled.get(i) {
                let bar_color = if i == 0 {
                    readout_color
                } else {
                    accent_color
                };
                painter.rect_filled(*fill, Rounding::ZERO, bar_color);
            }
        }

        // Connecting spectral envelope line
        let poly = self.envelope_polyline(rect, spacing);
        if poly.len() >= 2 {
            for win in poly.windows(2) {
                painter.line_segment([win[0], win[1]], Stroke::new(1.5, readout_color));
            }
        }

        // Header info: Fundamental and Q
        let info = format!("{:.0} Hz | Q {:.2}", self.fundamental_hz, self.resonance_q);
        painter.text(
            Pos2::new(rect.left() + 4.0, rect.top() + 4.0),
            Align2::LEFT_TOP,
            &info,
            FontId::monospace(9.0),
            readout_color,
        );
    }

    /// Renders the interactive resonant spectrum bar widget.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        accent_color: Color32,
        readout_color: Color32,
    ) -> Response {
        let desired_size = Vec2::new(160.0, 72.0);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());

        if response.clicked() || response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let spacing = 3.0;
                let slots = self.bar_rects(rect, spacing);
                for (i, slot) in slots.iter().enumerate() {
                    if pos.x >= slot.left() && pos.x <= slot.right() {
                        let norm = ((slot.bottom() - pos.y) / slot.height()).clamp(0.0, 1.0);
                        self.set_partial_weight(i, norm);
                        break;
                    }
                }
            }
        }

        if ui.is_rect_visible(rect) {
            self.paint(ui.painter(), rect, accent_color, readout_color);
        }

        response
    }
}

/// Standalone percussion vector widget for rendering resonant harmonic spectrum bars.
pub fn resonant_spectrum_bar_widget(
    ui: &mut Ui,
    model: &mut ResonantSpectrumBarModel,
    accent_color: Color32,
    readout_color: Color32,
) -> Response {
    model.show(ui, accent_color, readout_color)
}

/// Headless vector model for the Nord Drum 2 Tone vs Noise balance crossfader,
/// featuring center-detent snap, soft-takeover pickup, and dual-generator color splits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToneNoiseBalanceModel {
    /// Normalized balance position in 0.0..=1.0 (0.0 = 100% Tone, 0.5 = 50/50, 1.0 = 100% Noise).
    pub value: f32,
    /// Physical hardware controller position in 0.0..=1.0, if connected.
    pub physical_value: Option<f32>,
    /// Whether the physical control has caught up with the patch target value.
    pub is_caught: bool,
    /// Pickup threshold for soft takeover (default 0.03).
    pub catch_threshold: f32,
    /// Center detent deadzone width (default 0.02) where balance snaps cleanly to 0.50.
    pub center_deadzone: f32,
    /// Label for the Tone side (default "TONE").
    pub tone_label: String,
    /// Label for the Noise side (default "NOISE").
    pub noise_label: String,
}

/// Type alias matching the widget function naming for Tone/Noise balance sliders.
pub type ToneNoiseBalanceSliderModel = ToneNoiseBalanceModel;

impl Default for ToneNoiseBalanceModel {
    fn default() -> Self {
        Self {
            value: 0.5,
            physical_value: None,
            is_caught: true,
            catch_threshold: 0.03,
            center_deadzone: 0.02,
            tone_label: "TONE".to_string(),
            noise_label: "NOISE".to_string(),
        }
    }
}

impl ToneNoiseBalanceModel {
    /// Creates a new tone/noise balance model with initial normalized balance in 0.0..=1.0.
    #[must_use]
    pub fn new(value: f32) -> Self {
        let mut model = Self::default();
        model.set_balance(value);
        model
    }

    /// Sets custom side labels for Tone and Noise.
    #[must_use]
    pub fn with_labels(mut self, tone: impl Into<String>, noise: impl Into<String>) -> Self {
        self.tone_label = tone.into();
        self.noise_label = noise.into();
        self
    }

    /// Configures initial physical controller position for soft takeover.
    #[must_use]
    pub fn with_physical_value(mut self, physical: f32) -> Self {
        let phys = physical.clamp(0.0, 1.0);
        self.physical_value = Some(phys);
        let dist = (phys - self.value).abs();
        self.is_caught = dist <= self.catch_threshold;
        self
    }

    /// Sets custom pickup catch threshold.
    #[must_use]
    pub fn with_catch_threshold(mut self, threshold: f32) -> Self {
        self.catch_threshold = threshold.max(0.001);
        self
    }

    /// Sets custom center detent deadzone.
    #[must_use]
    pub fn with_center_deadzone(mut self, deadzone: f32) -> Self {
        self.center_deadzone = deadzone.max(0.0);
        self
    }

    /// Returns the Tone contribution fraction in 0.0..=1.0.
    #[must_use]
    pub fn tone_ratio(&self) -> f32 {
        1.0 - self.value
    }

    /// Returns the Noise contribution fraction in 0.0..=1.0.
    #[must_use]
    pub fn noise_ratio(&self) -> f32 {
        self.value
    }

    /// Returns the Tone percentage (0..100).
    #[must_use]
    pub fn tone_percentage(&self) -> u32 {
        ((1.0 - self.value) * 100.0).round() as u32
    }

    /// Returns the Noise percentage (0..100).
    #[must_use]
    pub fn noise_percentage(&self) -> u32 {
        (self.value * 100.0).round() as u32
    }

    /// Sets balance position with center detent snap.
    pub fn set_balance(&mut self, target: f32) {
        let clamped = target.clamp(0.0, 1.0);
        if (clamped - 0.5).abs() <= self.center_deadzone {
            self.value = 0.5;
        } else {
            self.value = clamped;
        }
    }

    /// Updates physical hardware position and evaluates soft takeover.
    pub fn update_physical_value(&mut self, new_phys: f32) -> SoftTakeoverOutcome {
        let clamped = new_phys.clamp(0.0, 1.0);
        self.physical_value = Some(clamped);

        if self.is_caught {
            self.set_balance(clamped);
            SoftTakeoverOutcome::Caught { new_value: self.value }
        } else {
            let dist = (clamped - self.value).abs();
            if dist <= self.catch_threshold {
                self.is_caught = true;
                self.set_balance(clamped);
                SoftTakeoverOutcome::Caught { new_value: self.value }
            } else {
                SoftTakeoverOutcome::PendingCatch { distance: dist }
            }
        }
    }

    /// Computes the slider track groove rectangle.
    #[must_use]
    pub fn track_rect(&self, total_rect: Rect) -> Rect {
        let h = 12.0;
        let margin_x = 10.0;
        Rect::from_min_size(
            Pos2::new(total_rect.left() + margin_x, total_rect.center().y - h * 0.5),
            Vec2::new((total_rect.width() - margin_x * 2.0).max(1.0), h),
        )
    }

    /// Computes the center detent position.
    #[must_use]
    pub fn center_notch_pos(&self, track: Rect) -> Pos2 {
        track.center()
    }

    /// Computes the Tone (left) portion of the track.
    #[must_use]
    pub fn tone_fill_rect(&self, track: Rect) -> Rect {
        let center_x = track.center().x;
        let thumb_x = track.left() + track.width() * self.value;
        if thumb_x < center_x {
            Rect::from_min_max(Pos2::new(thumb_x, track.top()), Pos2::new(center_x, track.bottom()))
        } else {
            Rect::from_min_max(Pos2::new(track.left(), track.top()), Pos2::new(center_x, track.bottom()))
        }
    }

    /// Computes the Noise (right) portion of the track.
    #[must_use]
    pub fn noise_fill_rect(&self, track: Rect) -> Rect {
        let center_x = track.center().x;
        let thumb_x = track.left() + track.width() * self.value;
        if thumb_x > center_x {
            Rect::from_min_max(Pos2::new(center_x, track.top()), Pos2::new(thumb_x, track.bottom()))
        } else {
            Rect::from_min_max(Pos2::new(center_x, track.top()), Pos2::new(track.right(), track.bottom()))
        }
    }

    /// Computes active slider thumb rectangle.
    #[must_use]
    pub fn thumb_rect(&self, track: Rect, thumb_width: f32) -> Rect {
        let thumb_x = track.left() + track.width() * self.value;
        let thumb_h = track.height() + 8.0;
        Rect::from_center_size(Pos2::new(thumb_x, track.center().y), Vec2::new(thumb_width, thumb_h))
    }

    /// Computes ghost thumb indicator rectangle if soft takeover is pending.
    #[must_use]
    pub fn ghost_thumb_rect(&self, track: Rect, thumb_width: f32) -> Option<Rect> {
        if self.is_caught {
            None
        } else {
            self.physical_value.map(|phys| {
                let ghost_x = track.left() + track.width() * phys;
                let thumb_h = track.height() + 8.0;
                Rect::from_center_size(Pos2::new(ghost_x, track.center().y), Vec2::new(thumb_width, thumb_h))
            })
        }
    }

    /// Paints the tone/noise balance slider.
    pub fn paint(
        &self,
        painter: &Painter,
        rect: Rect,
        tone_color: Color32,
        noise_color: Color32,
    ) {
        // Track groove
        let track = self.track_rect(rect);
        painter.rect_filled(track, Rounding::ZERO, Color32::from_gray(24));
        painter.rect_stroke(track, Rounding::ZERO, Stroke::new(1.0, Color32::from_gray(50)));

        // Tone half (left)
        let tone_half = Rect::from_min_max(track.min, Pos2::new(track.center().x, track.bottom()));
        let tone_alpha = ((1.0 - self.value) * 180.0) as u8 + 20;
        let tone_fill = Color32::from_rgba_premultiplied(
            tone_color.r() / 3,
            tone_color.g() / 3,
            tone_color.b() / 3,
            tone_alpha,
        );
        painter.rect_filled(tone_half, Rounding::ZERO, tone_fill);

        // Noise half (right)
        let noise_half = Rect::from_min_max(Pos2::new(track.center().x, track.top()), track.max);
        let noise_alpha = (self.value * 180.0) as u8 + 20;
        let noise_fill = Color32::from_rgba_premultiplied(
            noise_color.r() / 3,
            noise_color.g() / 3,
            noise_color.b() / 3,
            noise_alpha,
        );
        painter.rect_filled(noise_half, Rounding::ZERO, noise_fill);

        // Center detent vertical notch
        let notch = self.center_notch_pos(track);
        painter.line_segment(
            [
                Pos2::new(notch.x, track.top() - 3.0),
                Pos2::new(notch.x, track.bottom() + 3.0),
            ],
            Stroke::new(1.5, Color32::from_gray(140)),
        );

        // Ghost thumb indicator for soft takeover
        if let Some(ghost) = self.ghost_thumb_rect(track, 10.0) {
            painter.rect_stroke(ghost, Rounding::ZERO, Stroke::new(1.5, Color32::from_rgb(255, 179, 0)));
        }

        // Active Thumb Handle
        let thumb = self.thumb_rect(track, 12.0);
        let thumb_color = if self.value < 0.48 {
            tone_color
        } else if self.value > 0.52 {
            noise_color
        } else {
            Color32::WHITE
        };
        painter.rect_filled(thumb, Rounding::ZERO, Color32::from_gray(40));
        painter.rect_stroke(thumb, Rounding::ZERO, Stroke::new(1.5, thumb_color));

        // Percentage text labels above track
        let tone_text = format!("{} {}%", self.tone_label, self.tone_percentage());
        let noise_text = format!("{}% {}", self.noise_percentage(), self.noise_label);
        painter.text(
            Pos2::new(track.left(), rect.top() + 4.0),
            Align2::LEFT_TOP,
            &tone_text,
            FontId::proportional(10.0),
            tone_color,
        );
        painter.text(
            Pos2::new(track.right(), rect.top() + 4.0),
            Align2::RIGHT_TOP,
            &noise_text,
            FontId::proportional(10.0),
            noise_color,
        );
    }

    /// Renders the interactive tone/noise balance slider widget.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        tone_color: Color32,
        noise_color: Color32,
    ) -> Response {
        let desired_size = Vec2::new(160.0, 48.0);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());

        if response.clicked() || response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let track = self.track_rect(rect);
                let norm = ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0);
                self.set_balance(norm);
            }
        }

        if ui.is_rect_visible(rect) {
            self.paint(ui.painter(), rect, tone_color, noise_color);
        }

        response
    }
}

/// Standalone percussion vector widget for rendering the Tone vs Noise balance crossfader.
pub fn tone_noise_balance_slider(
    ui: &mut Ui,
    model: &mut ToneNoiseBalanceModel,
    tone_color: Color32,
    noise_color: Color32,
) -> Response {
    model.show(ui, tone_color, noise_color)
}

/// Headless vector badge model for the Nord Drum 2 voice channel selector,
/// displaying channel index, voice name, CC 70 focus state, and Solo/Mute LED indicators.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceFocusBadgeModel {
    /// Zero-based channel index (0..5 for Voices 1..6).
    pub voice_index: usize,
    /// Channel display name (for example "Kick", "Snare", "Hi-Hat").
    pub voice_name: String,
    /// Whether this voice currently holds editing focus via CC 70.
    pub is_focused: bool,
    /// Whether this voice is soloed.
    pub is_solo: bool,
    /// Whether this voice is muted.
    pub is_muted: bool,
    /// Assigned MIDI channel (1..16).
    pub midi_channel: u8,
    /// Assigned MIDI trigger note number (default 60 + voice_index).
    pub midi_note: u8,
    /// Continuous controller 70 index value (equal to voice_index as u8).
    pub cc70_value: u8,
}

impl VoiceFocusBadgeModel {
    /// Creates a new voice focus badge model for a voice channel index in 0..5.
    #[must_use]
    pub fn new(voice_index: usize, name: impl Into<String>) -> Self {
        let idx = voice_index.min(5);
        Self {
            voice_index: idx,
            voice_name: name.into(),
            is_focused: false,
            is_solo: false,
            is_muted: false,
            midi_channel: (idx as u8) + 1,
            midi_note: 60 + (idx as u8),
            cc70_value: idx as u8,
        }
    }

    /// Configures MIDI channel and trigger note.
    #[must_use]
    pub fn with_midi(mut self, channel: u8, note: u8) -> Self {
        self.midi_channel = channel.clamp(1, 16);
        self.midi_note = note;
        self
    }

    /// Sets initial focus state.
    #[must_use]
    pub fn with_focus(mut self, focused: bool) -> Self {
        self.is_focused = focused;
        self
    }

    /// Sets initial solo state.
    #[must_use]
    pub fn with_solo(mut self, solo: bool) -> Self {
        self.is_solo = solo;
        self
    }

    /// Sets initial mute state.
    #[must_use]
    pub fn with_muted(mut self, muted: bool) -> Self {
        self.is_muted = muted;
        self
    }

    /// Sets editing focus state.
    pub fn set_focus(&mut self, focused: bool) {
        self.is_focused = focused;
    }

    /// Toggles hardware solo state.
    pub fn toggle_solo(&mut self) -> bool {
        self.is_solo = !self.is_solo;
        self.is_solo
    }

    /// Toggles hardware mute state.
    pub fn toggle_mute(&mut self) -> bool {
        self.is_muted = !self.is_muted;
        self.is_muted
    }

    /// Returns 1-based voice number in 1..6.
    #[must_use]
    pub fn voice_number(&self) -> usize {
        self.voice_index + 1
    }

    /// Returns formatted display title string (for example "VOICE 1: KICK").
    #[must_use]
    pub fn display_title(&self) -> String {
        format!("VOICE {}: {}", self.voice_number(), self.voice_name.to_uppercase())
    }

    /// Computes layout rectangles for a horizontal strip of voice badges.
    #[must_use]
    pub fn compute_voice_strip_rects(total_rect: Rect, count: usize, spacing: f32) -> Vec<Rect> {
        if count == 0 {
            return Vec::new();
        }
        let badge_width = ((total_rect.width() - spacing * (count - 1) as f32) / count as f32).max(1.0);
        let mut rects = Vec::with_capacity(count);
        for i in 0..count {
            let left = total_rect.left() + i as f32 * (badge_width + spacing);
            rects.push(Rect::from_min_size(
                Pos2::new(left, total_rect.top()),
                Vec2::new(badge_width, total_rect.height()),
            ));
        }
        rects
    }

    /// Computes rectangles for focus LED, solo LED, and mute LED indicators inside badge_rect.
    #[must_use]
    pub fn led_indicator_rects(&self, badge_rect: Rect) -> (Rect, Rect, Rect) {
        let led_size = Vec2::new(8.0, 8.0);
        let y_top = badge_rect.top() + 6.0;
        let y_bot = badge_rect.bottom() - 14.0;

        let focus_led = Rect::from_min_size(Pos2::new(badge_rect.left() + 6.0, y_top), led_size);
        let solo_led = Rect::from_min_size(Pos2::new(badge_rect.left() + 6.0, y_bot), led_size);
        let mute_led = Rect::from_min_size(Pos2::new(badge_rect.left() + 18.0, y_bot), led_size);

        (focus_led, solo_led, mute_led)
    }

    /// Paints the voice focus badge.
    pub fn paint(
        &self,
        painter: &Painter,
        rect: Rect,
        accent_color: Color32,
        readout_color: Color32,
    ) {
        let (bg_fill, border_stroke) = if self.is_focused {
            (
                Color32::from_rgb(45, 14, 14),
                Stroke::new(2.0, accent_color),
            )
        } else {
            (
                Color32::from_gray(26),
                Stroke::new(1.0, Color32::from_gray(55)),
            )
        };

        painter.rect_filled(rect, Rounding::ZERO, bg_fill);
        painter.rect_stroke(rect, Rounding::ZERO, border_stroke);

        let (focus_led, solo_led, mute_led) = self.led_indicator_rects(rect);

        // Focus LED (Amber #FFB300 when focused)
        let focus_color = if self.is_focused {
            readout_color
        } else {
            Color32::from_gray(40)
        };
        painter.rect_filled(focus_led, Rounding::ZERO, focus_color);
        painter.rect_stroke(focus_led, Rounding::ZERO, Stroke::new(0.5, Color32::BLACK));

        // Solo LED (Amber #FFB000 when soloed)
        let solo_color = if self.is_solo {
            Color32::from_rgb(255, 176, 0)
        } else {
            Color32::from_gray(35)
        };
        painter.rect_filled(solo_led, Rounding::ZERO, solo_color);

        // Mute LED (Red #FF2020 when muted)
        let mute_color = if self.is_muted {
            Color32::from_rgb(255, 32, 32)
        } else {
            Color32::from_gray(35)
        };
        painter.rect_filled(mute_led, Rounding::ZERO, mute_color);

        // Voice Title
        let title_color = if self.is_focused {
            Color32::WHITE
        } else {
            Color32::from_gray(180)
        };
        painter.text(
            Pos2::new(focus_led.right() + 6.0, rect.top() + 5.0),
            Align2::LEFT_TOP,
            self.display_title(),
            FontId::proportional(11.0),
            title_color,
        );

        // MIDI channel and trigger note
        let midi_info = format!("CH {} | N{}", self.midi_channel, self.midi_note);
        painter.text(
            Pos2::new(mute_led.right() + 8.0, rect.bottom() - 14.0),
            Align2::LEFT_TOP,
            &midi_info,
            FontId::monospace(9.0),
            Color32::from_gray(140),
        );

        // CC 70 indicator badge
        let cc_text = format!("CC70:{}", self.cc70_value);
        painter.text(
            Pos2::new(rect.right() - 6.0, rect.bottom() - 14.0),
            Align2::RIGHT_TOP,
            &cc_text,
            FontId::monospace(8.0),
            if self.is_focused { readout_color } else { Color32::from_gray(100) },
        );
    }

    /// Renders the interactive voice focus badge widget.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        accent_color: Color32,
        readout_color: Color32,
    ) -> Response {
        let desired_size = Vec2::new(120.0, 44.0);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

        if response.clicked() {
            self.set_focus(true);
        }

        if ui.is_rect_visible(rect) {
            self.paint(ui.painter(), rect, accent_color, readout_color);
        }

        response
    }
}

/// Standalone percussion vector widget for rendering a voice focus selector badge.
pub fn voice_focus_badge(
    ui: &mut Ui,
    model: &mut VoiceFocusBadgeModel,
    accent_color: Color32,
    readout_color: Color32,
) -> Response {
    model.show(ui, accent_color, readout_color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotary_knob_angle_calculations() {
        let knob = RotaryKnobModel::new("filter_cutoff", "Cutoff", 0.5);
        let center_angle = knob.current_angle();
        assert!((center_angle - 0.0).abs() < 1e-4);

        let min_angle = knob.angle_for_norm(0.0);
        let max_angle = knob.angle_for_norm(1.0);
        assert!((min_angle - (-135.0_f32.to_radians())).abs() < 1e-4);
        assert!((max_angle - 135.0_f32.to_radians()).abs() < 1e-4);
    }

    #[test]
    fn rotary_knob_soft_takeover_catch_logic() {
        let mut knob = RotaryKnobModel::new("resonance", "Resonance", 0.70)
            .with_catch_threshold(0.05)
            .with_physical_value(0.10);

        assert!(!knob.is_caught);
        assert_eq!(knob.value, 0.70);

        // Movement below threshold stays pending
        let res1 = knob.update_physical_value(0.30);
        assert!(matches!(res1, SoftTakeoverOutcome::PendingCatch { .. }));
        assert!(!knob.is_caught);
        assert_eq!(knob.value, 0.70);

        // Movement crossing or reaching threshold catches the control
        let res2 = knob.update_physical_value(0.68);
        assert!(matches!(res2, SoftTakeoverOutcome::Caught { new_value } if (new_value - 0.68).abs() < 1e-4));
        assert!(knob.is_caught);
        assert_eq!(knob.value, 0.68);

        // After catch, changes track directly
        let res3 = knob.update_physical_value(0.85);
        assert!(matches!(res3, SoftTakeoverOutcome::Caught { new_value } if (new_value - 0.85).abs() < 1e-4));
        assert_eq!(knob.value, 0.85);
    }

    #[test]
    fn rotary_knob_ghost_ring_points_present_only_when_pending() {
        let center = Pos2::new(100.0, 100.0);
        let mut knob = RotaryKnobModel::new("lfo_speed", "Speed", 0.80)
            .with_physical_value(0.20);

        assert!(!knob.is_caught);
        let ghost_pts = knob.ghost_ring_points(center, 40.0, 16);
        assert!(ghost_pts.is_some());
        assert!(!ghost_pts.unwrap().is_empty());

        let ghost_line = knob.ghost_pointer_endpoints(center, 10.0, 30.0);
        assert!(ghost_line.is_some());

        // Catch the knob
        knob.update_physical_value(0.80);
        assert!(knob.is_caught);
        assert!(knob.ghost_ring_points(center, 40.0, 16).is_none());
        assert!(knob.ghost_pointer_endpoints(center, 10.0, 30.0).is_none());
    }

    #[test]
    fn step_led_button_state_and_toggles() {
        let mut step = StepLedButtonModel::new(0).with_note("C", 0);
        assert_eq!(step.step_number, 1);
        assert!(!step.gate_on);
        assert_eq!(step.led_state, StepLedState::Off);

        step.toggle_gate();
        assert!(step.gate_on);
        assert_eq!(step.led_state, StepLedState::On);

        step.toggle_accent();
        assert!(step.accent);

        step.toggle_slide();
        assert!(step.slide);

        step.set_current_step(true);
        assert_eq!(step.led_state, StepLedState::Blinking);
    }

    #[test]
    fn step_led_button_rects_spacing() {
        let total = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(160.0, 40.0));
        let rects = StepLedButtonModel::step_rects(total, 4, 4.0);
        assert_eq!(rects.len(), 4);
        assert_eq!(rects[0].left(), 0.0);
        assert!((rects[3].right() - 160.0).abs() < 1e-3);
    }

    #[test]
    fn envelope_curve_evaluation_and_proportions() {
        let env = EnvelopeCurveModel::new(0.10, 0.20, 0.50, 0.30);
        let (fa, fd, fh, fr) = env.stage_fractions();
        assert!(((fa + fd + fh + fr) - 1.0).abs() < 1e-4);

        // At t = 0, envelope is at zero
        assert!((env.evaluate_at(0.0) - 0.0).abs() < 1e-4);

        // At attack peak, amplitude reaches 1.0
        let peak_amp = env.evaluate_at(fa);
        assert!((peak_amp - 1.0).abs() < 1e-3);

        // At sustain phase, amplitude is equal to sustain level
        let sustain_amp = env.evaluate_at(fa + fd + fh * 0.5);
        assert!((sustain_amp - 0.50).abs() < 1e-3);

        // At t = 1.0, release reaches 0.0
        assert!((env.evaluate_at(1.0) - 0.0).abs() < 1e-4);
    }

    #[test]
    fn envelope_curve_polygon_generation() {
        let env = EnvelopeCurveModel::default();
        let frame = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(200.0, 100.0));
        let polyline = env.generate_polyline(frame, 32);
        assert_eq!(polyline.len(), 33);

        let polygon = env.generate_filled_polygon(frame, 32);
        assert_eq!(polygon.len(), 35); // 33 curve points + bottom right + bottom left
    }

    #[test]
    fn key_overlay_badge_bs2_25_keys_layout() {
        let keys = KeyOverlayBadgeModel::bs2_25_keys();
        assert_eq!(keys.len(), 25);
        assert_eq!(keys[0].note_name, "C");
        assert_eq!(keys[0].function_label.as_deref(), Some("OSC 1"));
        assert_eq!(keys[24].note_name, "C");
        assert_eq!(keys[24].function_label.as_deref(), Some("AFX / PARA"));

        let total_rect = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(300.0, 100.0));
        let (white_rects, black_rects) = KeyOverlayBadgeModel::calculate_25_key_layout(total_rect);
        assert_eq!(white_rects.len(), 15);
        assert_eq!(black_rects.len(), 10);
        assert_eq!(white_rects.len() + black_rects.len(), 25);
    }

    #[test]
    fn drum_pad_velocity_trigger_and_decay() {
        let mut pad = DrumPadVelocityModel::new(0, "Kick");
        assert_eq!(pad.channel_index, 0);
        assert_eq!(pad.voice_number(), 1);
        assert_eq!(pad.midi_note, 60);
        assert_eq!(pad.velocity, 0);
        assert_eq!(pad.peak_velocity, 0);
        assert!(!pad.is_triggered);
        assert_eq!(pad.normalized_velocity(), 0.0);
        assert_eq!(pad.normalized_peak(), 0.0);

        // Strike pad with velocity 110
        pad.trigger(110);
        assert!(pad.is_triggered);
        assert_eq!(pad.velocity, 110);
        assert_eq!(pad.peak_velocity, 110);
        assert!((pad.normalized_velocity() - (110.0 / 127.0)).abs() < 1e-4);

        // Subsequent strike with lower velocity 80 does not lower peak
        pad.trigger(80);
        assert_eq!(pad.velocity, 80);
        assert_eq!(pad.peak_velocity, 110);

        // Release trigger
        pad.release();
        assert!(!pad.is_triggered);
        assert_eq!(pad.velocity, 80);

        // Peak decay
        pad.decay_peak(30);
        assert_eq!(pad.peak_velocity, 80);
        pad.decay_peak(100);
        assert_eq!(pad.peak_velocity, 0);
    }

    #[test]
    fn drum_pad_velocity_rect_geometry() {
        let total = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(300.0, 200.0));
        let pads = DrumPadVelocityModel::compute_pad_rects(total, 6, 3, 6.0);
        assert_eq!(pads.len(), 6);
        for p in &pads {
            assert!(p.width() > 50.0);
            assert!(p.height() > 50.0);
            assert!(total.contains_rect(*p));
        }

        let model = DrumPadVelocityModel::new(0, "Snare").with_velocity(64);
        let bar = model.velocity_bar_rect(pads[0]);
        let fill = model.velocity_fill_rect(pads[0]);
        assert!(pads[0].contains_rect(bar));
        assert!(bar.contains_rect(fill));
        assert!((fill.width() - bar.width() * (64.0 / 127.0)).abs() < 1.0);
    }

    #[test]
    fn resonant_spectrum_bar_presets_and_harmonics() {
        let bell = ResonantSpectrumBarModel::from_preset("bell");
        assert_eq!(bell.partial_count, 8);
        assert_eq!(bell.harmonic_weights.len(), 8);
        assert_eq!(bell.fundamental_hz, 520.0);
        assert_eq!(bell.resonance_q, 0.90);

        let membrane = ResonantSpectrumBarModel::from_preset("membrane");
        assert_eq!(membrane.fundamental_hz, 140.0);
        assert!(membrane.harmonic_weights[0] > membrane.harmonic_weights[7]);

        let mut custom = ResonantSpectrumBarModel::new(10)
            .with_fundamental(440.0)
            .with_resonance_q(0.80)
            .with_spectral_tilt(-0.20)
            .with_decay_damping(0.40);
        assert_eq!(custom.partial_count, 10);
        custom.recompute_harmonics();
        for w in &custom.harmonic_weights {
            assert!(*w >= 0.0 && *w <= 1.0);
        }

        custom.set_partial_weight(3, 0.95);
        assert_eq!(custom.harmonic_weights[3], 0.95);
    }

    #[test]
    fn resonant_spectrum_bar_geometry() {
        let model = ResonantSpectrumBarModel::from_preset("cymbal");
        let total = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(240.0, 80.0));
        let slots = model.bar_rects(total, 4.0);
        assert_eq!(slots.len(), 8);

        let filled = model.filled_bar_rects(total, 4.0);
        assert_eq!(filled.len(), 8);
        for (i, fill) in filled.iter().enumerate() {
            assert!(slots[i].contains_rect(*fill));
            assert!(fill.height() <= slots[i].height());
        }

        let poly = model.envelope_polyline(total, 4.0);
        assert_eq!(poly.len(), 8);
    }

    #[test]
    fn tone_noise_balance_center_detent_and_percentages() {
        let mut balance = ToneNoiseBalanceModel::new(0.50);
        assert_eq!(balance.value, 0.50);
        assert_eq!(balance.tone_ratio(), 0.50);
        assert_eq!(balance.noise_ratio(), 0.50);
        assert_eq!(balance.tone_percentage(), 50);
        assert_eq!(balance.noise_percentage(), 50);

        // Center detent snapping within 0.02 deadzone (0.49 snaps to 0.50)
        balance.set_balance(0.49);
        assert_eq!(balance.value, 0.50);
        balance.set_balance(0.515);
        assert_eq!(balance.value, 0.50);

        // Pure Tone (0.0)
        balance.set_balance(0.0);
        assert_eq!(balance.value, 0.0);
        assert_eq!(balance.tone_percentage(), 100);
        assert_eq!(balance.noise_percentage(), 0);

        // Tone dominant (0.20)
        balance.set_balance(0.20);
        assert_eq!(balance.tone_percentage(), 80);
        assert_eq!(balance.noise_percentage(), 20);

        // Pure Noise (1.0)
        balance.set_balance(1.0);
        assert_eq!(balance.value, 1.0);
        assert_eq!(balance.tone_percentage(), 0);
        assert_eq!(balance.noise_percentage(), 100);
    }

    #[test]
    fn tone_noise_balance_soft_takeover_catch() {
        let mut balance = ToneNoiseBalanceModel::new(0.75)
            .with_catch_threshold(0.04)
            .with_physical_value(0.20);

        assert!(!balance.is_caught);
        assert_eq!(balance.value, 0.75);

        let total = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(200.0, 40.0));
        let track = balance.track_rect(total);
        assert!(balance.ghost_thumb_rect(track, 10.0).is_some());

        // Movement below threshold stays pending
        let res1 = balance.update_physical_value(0.40);
        assert!(matches!(res1, SoftTakeoverOutcome::PendingCatch { .. }));
        assert!(!balance.is_caught);
        assert_eq!(balance.value, 0.75);

        // Movement reaching threshold catches control
        let res2 = balance.update_physical_value(0.73);
        assert!(matches!(res2, SoftTakeoverOutcome::Caught { .. }));
        assert!(balance.is_caught);
        assert_eq!(balance.value, 0.73);
        assert!(balance.ghost_thumb_rect(track, 10.0).is_none());
    }

    #[test]
    fn voice_focus_badge_state_and_strip_layout() {
        let mut badges: Vec<VoiceFocusBadgeModel> = (0..6)
            .map(|i| VoiceFocusBadgeModel::new(i, format!("Voice {}", i + 1)))
            .collect();
        assert_eq!(badges.len(), 6);
        assert_eq!(badges[0].voice_number(), 1);
        assert_eq!(badges[0].cc70_value, 0);
        assert_eq!(badges[5].voice_number(), 6);
        assert_eq!(badges[5].cc70_value, 5);

        // Strip layout
        let total = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(600.0, 50.0));
        let rects = VoiceFocusBadgeModel::compute_voice_strip_rects(total, 6, 8.0);
        assert_eq!(rects.len(), 6);
        assert_eq!(rects[0].left(), 0.0);

        // Indicators
        let (focus_led, solo_led, mute_led) = badges[0].led_indicator_rects(rects[0]);
        assert!(rects[0].contains_rect(focus_led));
        assert!(rects[0].contains_rect(solo_led));
        assert!(rects[0].contains_rect(mute_led));

        // Focus and solo/mute toggles
        assert!(!badges[0].is_focused);
        badges[0].set_focus(true);
        assert!(badges[0].is_focused);

        assert!(!badges[0].is_solo);
        badges[0].toggle_solo();
        assert!(badges[0].is_solo);

        assert!(!badges[0].is_muted);
        badges[0].toggle_mute();
        assert!(badges[0].is_muted);
    }

    #[test]
    fn peak_vector_widgets_geometry_and_state() {
        let mut osc = WavetableOscillatorModel::new(1, 0.5, 0.25, 0);
        assert_eq!(osc.osc_index, 1);
        let frame = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(180.0, 60.0));
        let wave_pts = osc.waveform_polyline(frame, 32);
        assert_eq!(wave_pts.len(), 33);
        osc.set_shape(0.8);
        assert!((osc.shape - 0.8).abs() < 1e-4);

        let svf = StateVariableFilterModel::new(0.65, 0.40, 0.20, 0.15);
        let resp_pts = svf.frequency_response_polyline(frame, 32);
        assert_eq!(resp_pts.len(), 33);
        assert!(svf.is_within_safe_sweep());

        let mut anim1 = AnimateHoldButtonModel::new(1, 114, false);
        assert!(!anim1.held);
        anim1.toggle();
        assert!(anim1.held);
        assert_eq!(anim1.midi_cc_value(), 127);

        let mut voices = VoiceAllocationMeterModel::new();
        voices.set_active_notes(&[60, 64, 67]);
        assert_eq!(voices.active_count(), 3);
        let slots = voices.voice_slot_rects(frame, 4.0);
        assert_eq!(slots.len(), 8);
    }
}

// =============================================================================
// Novation Peak Custom Vector Widgets
// =============================================================================

/// Headless-testable vector model for a Novation Peak New Oxford Oscillator (NCO / Wavetable).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WavetableOscillatorModel {
    /// 1-based oscillator number (`1..=3`).
    pub osc_index: u8,
    /// Normalized wave shape / wavetable position (`0.0..=1.0`, `CC 12` / `CC 39` / `CC 71`).
    pub shape: f32,
    /// Normalized virtual sync depth (`0.0..=1.0`, `CC 34` / `CC 42` / `CC 44`).
    pub vsync: f32,
    /// Coarse pitch offset in semitones (`-64..=+63`).
    pub coarse_st: i32,
}

impl WavetableOscillatorModel {
    /// Creates a new New Oxford Oscillator waveform model.
    #[must_use]
    pub fn new(osc_index: u8, shape: f32, vsync: f32, coarse_st: i32) -> Self {
        Self {
            osc_index: osc_index.clamp(1, 3),
            shape: shape.clamp(0.0, 1.0),
            vsync: vsync.clamp(0.0, 1.0),
            coarse_st: coarse_st.clamp(-64, 63),
        }
    }

    /// Updates the normalized wavetable shape (`0.0..=1.0`).
    pub fn set_shape(&mut self, shape: f32) {
        self.shape = shape.clamp(0.0, 1.0);
    }

    /// Evaluates the synthesized NCO waveform amplitude in `-1.0..=1.0` at phase `t` (`0.0..=1.0`).
    #[must_use]
    pub fn evaluate_wave(&self, t: f32) -> f32 {
        let sync_mult = 1.0 + self.vsync * 3.0;
        let phase = (t.clamp(0.0, 1.0) * sync_mult).fract();
        let sine = (phase * std::f32::consts::TAU).sin();
        let saw = 2.0 * phase - 1.0;
        let pulse = if phase < (0.5 - 0.35 * (self.shape - 0.5)) {
            1.0
        } else {
            -1.0
        };
        if self.shape < 0.5 {
            let mix = self.shape * 2.0;
            sine * (1.0 - mix) + saw * mix
        } else {
            let mix = (self.shape - 0.5) * 2.0;
            saw * (1.0 - mix) + pulse * mix
        }
    }

    /// Generates `steps + 1` polyline points inside `rect` representing the oscillator waveform.
    #[must_use]
    pub fn waveform_polyline(&self, rect: Rect, steps: usize) -> Vec<Pos2> {
        let n = steps.max(8);
        let pad = 4.0;
        let w = (rect.width() - pad * 2.0).max(1.0);
        let h = (rect.height() - pad * 2.0).max(1.0);
        let mid_y = rect.center().y;
        (0..=n)
            .map(|i| {
                let frac = i as f32 / n as f32;
                let x = rect.left() + pad + frac * w;
                let y = mid_y - self.evaluate_wave(frac) * (h * 0.42);
                Pos2::new(x, y)
            })
            .collect()
    }
}

/// Renders a `WavetableOscillatorModel` vector display inside an `egui::Ui`.
pub fn wavetable_oscillator_widget(
    ui: &mut Ui,
    model: &WavetableOscillatorModel,
    accent: Color32,
    readout: Color32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(190.0, 64.0), Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, Rounding::ZERO, Color32::from_rgb(16, 20, 26));
        painter.rect_stroke(rect, Rounding::ZERO, Stroke::new(1.0, accent));
        let pts = model.waveform_polyline(rect, 48);
        painter.add(Shape::line(pts, Stroke::new(1.8, accent)));
        painter.text(
            rect.left_top() + Vec2::new(6.0, 4.0),
            Align2::LEFT_TOP,
            format!(
                "OSC {} | SHP {:.0}% | VSYNC {:.0}%",
                model.osc_index,
                model.shape * 100.0,
                model.vsync * 100.0
            ),
            FontId::monospace(10.0),
            readout,
        );
    }
    response
}

/// Headless-testable vector model for the Novation Peak Analog OTA State-Variable Filter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateVariableFilterModel {
    /// Normalized cutoff frequency (`0.0..=1.0`, `CC 29`).
    pub cutoff: f32,
    /// Normalized filter resonance (`0.0..=1.0`, `CC 79`, safe sweep ceiling `62/127 ~= 0.488`).
    pub resonance: f32,
    /// Normalized pre-filter overdrive (`0.0..=1.0`, `CC 80`, safe sweep ceiling `50/127 ~= 0.394`).
    pub overdrive: f32,
    /// Normalized post-filter drive (`0.0..=1.0`, `CC 36`, safe sweep ceiling `50/127 ~= 0.394`).
    pub post_drive: f32,
}

impl StateVariableFilterModel {
    /// Creates a new state-variable filter response curve model.
    #[must_use]
    pub fn new(cutoff: f32, resonance: f32, overdrive: f32, post_drive: f32) -> Self {
        Self {
            cutoff: cutoff.clamp(0.0, 1.0),
            resonance: resonance.clamp(0.0, 1.0),
            overdrive: overdrive.clamp(0.0, 1.0),
            post_drive: post_drive.clamp(0.0, 1.0),
        }
    }

    /// Returns `true` if resonance (`<= 62/127`) and drive (`<= 50/127`) are within acoustic `SafeSweepBounds`.
    #[must_use]
    pub fn is_within_safe_sweep(&self) -> bool {
        (self.resonance * 127.0).round() <= 62.0
            && (self.overdrive * 127.0).round() <= 50.0
            && (self.post_drive * 127.0).round() <= 50.0
    }

    /// Generates `steps + 1` polyline points inside `rect` representing the filter magnitude curve.
    #[must_use]
    pub fn frequency_response_polyline(&self, rect: Rect, steps: usize) -> Vec<Pos2> {
        let n = steps.max(8);
        let pad = 6.0;
        let w = (rect.width() - pad * 2.0).max(1.0);
        let h = (rect.height() - pad * 2.0).max(1.0);
        let cutoff_x = self.cutoff.clamp(0.05, 0.95);

        (0..=n)
            .map(|i| {
                let f = i as f32 / n as f32;
                let passband = if f <= cutoff_x {
                    0.55 + 0.15 * (self.overdrive + self.post_drive) * 0.5
                } else {
                    let roll = ((f - cutoff_x) / (1.0 - cutoff_x).max(0.05)).clamp(0.0, 1.0);
                    (0.55 * (1.0 - roll).powi(2)).max(0.02)
                };
                let dist = ((f - cutoff_x) / 0.08).powi(2);
                let res_peak = self.resonance * 0.42 * (-dist).exp();
                let mag = (passband + res_peak).clamp(0.0, 1.0);
                let x = rect.left() + pad + f * w;
                let y = rect.bottom() - pad - mag * h;
                Pos2::new(x, y)
            })
            .collect()
    }
}

/// Renders a `StateVariableFilterModel` vector display inside an `egui::Ui`.
pub fn state_variable_filter_widget(
    ui: &mut Ui,
    model: &StateVariableFilterModel,
    accent: Color32,
    readout: Color32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(240.0, 72.0), Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, Rounding::ZERO, Color32::from_rgb(16, 20, 26));
        painter.rect_stroke(rect, Rounding::ZERO, Stroke::new(1.0, accent));
        let pts = model.frequency_response_polyline(rect, 48);
        painter.add(Shape::line(pts, Stroke::new(2.0, accent)));
        let badge = if model.is_within_safe_sweep() {
            "SAFE SWEEP OK"
        } else {
            "HIGH RESO/DRIVE"
        };
        painter.text(
            rect.left_top() + Vec2::new(6.0, 4.0),
            Align2::LEFT_TOP,
            format!(
                "OTA SVF | CUT {:.0}% | RES {:.0}% | {}",
                model.cutoff * 100.0,
                model.resonance * 100.0,
                badge
            ),
            FontId::monospace(10.0),
            readout,
        );
    }
    response
}

/// Headless-testable model for the Novation Peak Animate 1 (`CC 114`) and Animate 2 (`CC 115`) hold buttons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimateHoldButtonModel {
    /// Button index (`1` or `2`).
    pub animate_index: u8,
    /// MIDI CC number (`114` for Animate 1, `115` for Animate 2).
    pub cc_number: u8,
    /// Whether the performance hold button is currently latched/held.
    pub held: bool,
}

impl AnimateHoldButtonModel {
    /// Creates a new `AnimateHoldButtonModel`.
    #[must_use]
    pub fn new(animate_index: u8, cc_number: u8, held: bool) -> Self {
        Self {
            animate_index: animate_index.clamp(1, 2),
            cc_number,
            held,
        }
    }

    /// Toggles the held state and returns the new state.
    pub fn toggle(&mut self) -> bool {
        self.held = !self.held;
        self.held
    }

    /// Returns the 7-bit MIDI CC value (`127` when held, `0` when released).
    #[must_use]
    pub const fn midi_cc_value(&self) -> u8 {
        if self.held {
            127
        } else {
            0
        }
    }
}

/// Renders an interactive `AnimateHoldButtonModel` inside an `egui::Ui`.
pub fn animate_hold_button_widget(
    ui: &mut Ui,
    model: &mut AnimateHoldButtonModel,
    accent: Color32,
    readout: Color32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(128.0, 30.0), Sense::click());
    if response.clicked() {
        model.toggle();
    }
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let fill = if model.held {
            accent
        } else {
            Color32::from_rgb(24, 28, 36)
        };
        let text_col = if model.held {
            Color32::WHITE
        } else {
            readout
        };
        painter.rect_filled(rect, Rounding::ZERO, fill);
        painter.rect_stroke(rect, Rounding::ZERO, Stroke::new(1.0, accent));
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            format!(
                "ANIMATE {} (CC {}) [{}]",
                model.animate_index,
                model.cc_number,
                if model.held { "ON" } else { "OFF" }
            ),
            FontId::monospace(10.5),
            text_col,
        );
    }
    response
}

/// Headless-testable model for the Novation Peak 8-Voice Polyphonic Allocation Meter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceAllocationMeterModel {
    /// Active MIDI note (`0..=127`) per hardware voice slot (`0..8`), or `None` if idle.
    pub voices: [Option<u8>; 8],
}

impl Default for VoiceAllocationMeterModel {
    fn default() -> Self {
        Self::new()
    }
}

impl VoiceAllocationMeterModel {
    /// Creates an idle 8-voice meter.
    #[must_use]
    pub const fn new() -> Self {
        Self { voices: [None; 8] }
    }

    /// Synchronizes the 8 voice slots from a slice of active MIDI notes.
    pub fn set_active_notes(&mut self, notes: &[u8]) {
        self.voices = [None; 8];
        for (idx, &note) in notes.iter().take(8).enumerate() {
            self.voices[idx] = Some(note & 0x7F);
        }
    }

    /// Returns the number of currently active voices (`0..=8`).
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.voices.iter().filter(|v| v.is_some()).count()
    }

    /// Computes 8 equally spaced bounding rectangles inside `total_rect`.
    #[must_use]
    pub fn voice_slot_rects(&self, total_rect: Rect, gap: f32) -> [Rect; 8] {
        let total_gap = gap * 7.0;
        let slot_w = ((total_rect.width() - total_gap) / 8.0).max(4.0);
        std::array::from_fn(|i| {
            let x = total_rect.left() + i as f32 * (slot_w + gap);
            Rect::from_min_size(Pos2::new(x, total_rect.top()), Vec2::new(slot_w, total_rect.height()))
        })
    }
}

/// Renders a `VoiceAllocationMeterModel` 8-voice LED bar inside an `egui::Ui`.
pub fn voice_allocation_meter_widget(
    ui: &mut Ui,
    model: &VoiceAllocationMeterModel,
    accent: Color32,
    readout: Color32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(260.0, 24.0), Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let slots = model.voice_slot_rects(rect, 4.0);
        for (i, slot_rect) in slots.iter().enumerate() {
            let active_note = model.voices[i];
            let fill = if active_note.is_some() {
                accent
            } else {
                Color32::from_rgb(22, 26, 32)
            };
            painter.rect_filled(*slot_rect, Rounding::ZERO, fill);
            painter.rect_stroke(*slot_rect, Rounding::ZERO, Stroke::new(1.0, accent));
            let label = match active_note {
                Some(n) => format!("V{}:{}", i + 1, n),
                None => format!("V{}", i + 1),
            };
            painter.text(
                slot_rect.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::monospace(9.0),
                if active_note.is_some() {
                    Color32::WHITE
                } else {
                    readout
                },
            );
        }
    }
    response
}
