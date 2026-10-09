//! Navigation policy and press-relative gestures over the numerical core
//! ([`crate::camera`], [`crate::controller`]).
//! No UI, window, scene or ECS types: adapters supply logical pixels and a picked
//! local-space anchor.
use crate::length::Length;
use crate::scale::PerPx;
use glam::{Quat, Vec2, Vec3};
use serde::{Deserialize, Serialize};

use crate::camera::{CameraPose, Projection};
use crate::controller::{CameraController, CameraControllerConfig, GestureKind};
use crate::momentum::MomentumSettings;
use crate::smoothing::SmoothingSettings;

/// Absolute logical-pixel position in the adapter's coordinate system.
/// A gesture's samples must use the same origin as its press.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenPoint(pub Vec2);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NavigationAction {
    Pan,
    Orbit,
    Disabled,
}

impl NavigationAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pan => "Pan",
            Self::Orbit => "Orbit",
            Self::Disabled => "Disabled",
        }
    }

    pub fn gesture(self) -> Option<GestureKind> {
        match self {
            Self::Pan => Some(GestureKind::Pan),
            Self::Orbit => Some(GestureKind::Orbit),
            Self::Disabled => None,
        }
    }
}

/// Motion permissions are independent of button bindings and projection.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnabledMotion {
    pub pan: bool,
    pub orbit: bool,
    pub zoom: bool,
}

impl Default for EnabledMotion {
    fn default() -> Self {
        Self {
            pan: true,
            orbit: true,
            zoom: true,
        }
    }
}

/// What a pan drags along with the pointer. In orthographic both are the same
/// motion; they differ in perspective, where the pan speed in world units per
/// pixel depends on the depth it is measured at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanGrip {
    /// The point under the pointer at the press — surface, ground or the
    /// previous depth, as the host resolves it — stays under the pointer for
    /// the whole hold, and becomes the pivot. Nearer and farther points move
    /// with parallax around it, as in CAD packages.
    #[default]
    Pointer,
    /// The view-centre pivot's depth sets the pan speed, and the pivot stays
    /// at the view centre. Points at another depth slide under the pointer in
    /// perspective.
    PivotDepth,
}

impl PanGrip {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pointer => "Point under cursor",
            Self::PivotDepth => "Pivot depth",
        }
    }
}

/// View cube size step: Small/Medium/Large.
/// The pixel layout belongs to the host; this crate only
/// stores the viewer's choice next to the other overlay setting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewCubeSize {
    /// No face labels: the letters would be about 6 px.
    Small,
    #[default]
    Medium,
    Large,
}

impl ViewCubeSize {
    pub const ALL: [Self; 3] = [Self::Small, Self::Medium, Self::Large];

    pub fn label(self) -> &'static str {
        match self {
            Self::Small => "Small",
            Self::Medium => "Medium",
            Self::Large => "Large",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum FallbackDepth {
    #[default]
    Previous,
    Fixed(Length),
}

/// Values stay f64 in the host's model units until the local f32 camera boundary.
/// Limits constrain zoom samples, not restore, resize or view commands.
///
/// The fields are serialized into host documents as
/// bare numbers (`PerPx`/`Length` are `serde(transparent)`). Deserialization does
/// not go through `PerPx::new`/`Length::new`: a file value is checked by
/// [`Self::validate`] at the file boundary instead of panicking on load.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NavigationLimits {
    pub min_scale: PerPx,
    pub max_scale: PerPx,
    pub initial_depth: Length,
    pub fallback_depth: FallbackDepth,
}

impl Default for NavigationLimits {
    fn default() -> Self {
        Self {
            min_scale: PerPx(0.05),
            max_scale: PerPx(50.0),
            initial_depth: Length(10000.0),
            fallback_depth: FallbackDepth::Previous,
        }
    }
}

impl NavigationLimits {
    pub fn validate(self) -> Result<(), String> {
        // Keep the local f32 calculations within a bounded numerical envelope.
        for value in [self.min_scale.0, self.max_scale.0] {
            if !value.is_finite() || !(0.001..=10000.0).contains(&value) {
                return Err(
                    "camera zoom limits must be finite and in 0.001..=10000 model units/px".into(),
                );
            }
        }
        if self.min_scale >= self.max_scale {
            return Err("camera minimum scale must be less than maximum scale".into());
        }
        for depth in [
            Some(self.initial_depth),
            match self.fallback_depth {
                FallbackDepth::Previous => None,
                FallbackDepth::Fixed(depth) => Some(depth),
            },
        ]
        .into_iter()
        .flatten()
        {
            if !depth.0.is_finite() || !(0.01..=1e7).contains(&depth.0) {
                return Err(
                    "camera depth must be finite and in 0.01..=10000000 model units".into(),
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NavigationSettings {
    pub middle: NavigationAction,
    pub shift_middle: NavigationAction,
    pub secondary: NavigationAction,
    pub space_primary: NavigationAction,
    /// Dimensionless multipliers relative to the shared core's angular speed
    /// and our logical-pixel wheel convention.
    pub orbit_speed: f32,
    pub zoom_speed: f32,
    /// World-up orbit keeps the initial roll rather than accumulating new roll.
    /// Canonical view commands can level an intentionally rolled saved view.
    pub keep_horizon: bool,
    pub invert_zoom: bool,
    /// Screen-space overlay, independent of projection, scale and body depth.
    #[serde(default = "orbit_anchor_visible")]
    pub show_orbit_anchor: bool,
    /// Files saved before this setting existed load with the default grip.
    #[serde(default)]
    pub pan_grip: PanGrip,
    #[serde(default)]
    pub enabled: EnabledMotion,
    #[serde(default)]
    pub limits: NavigationLimits,
    #[serde(default)]
    pub smoothing: SmoothingSettings,
    /// Inertia after releasing pan/orbit, off by default. Files saved before
    /// this setting existed load with it off.
    #[serde(default)]
    pub momentum: MomentumSettings,
    /// Vertical orbit speed relative to the horizontal `orbit_speed`.
    /// A ratio rather than a second
    /// speed, so files saved before it existed load with 1 — Y = X at any
    /// `orbit_speed`.
    #[serde(default = "unit_ratio")]
    pub orbit_vertical_ratio: f32,
    /// World-up orbit (`keep_horizon`) passes over the poles instead of
    /// stopping there. Past a pole the camera is
    /// upside down with a level horizon: no roll is added, and the next
    /// gesture tilts the way the screen shows. Off by default.
    #[serde(default)]
    pub pass_poles: bool,
    /// Wheel/pinch during a held pan or orbit zooms about the gesture's
    /// anchor. Enabled by default.
    #[serde(default = "zoom_in_hold_enabled")]
    pub zoom_in_hold: bool,
    /// In perspective, zooming in past `min_scale` moves the camera and
    /// its anchor forward together instead of stopping, so the camera can
    /// pass through a surface. Off by
    /// default; see [`Self::zoom_to`].
    #[serde(default)]
    pub zoom_through: bool,
    /// Alt+wheel changes perspective FOV with dolly at the pointed anchor.
    /// Legacy documents enable the new gesture; pinch still means zoom.
    #[serde(default = "fov_wheel_enabled")]
    pub fov_wheel: bool,
    /// Screen overlay like `show_orbit_anchor`. Files saved before this
    /// setting existed load with the default Medium.
    #[serde(default)]
    pub view_cube_size: ViewCubeSize,
    /// A held pan/orbit grabs and hides the pointer and is driven by raw
    /// mouse motion, so the screen edge does not stop the gesture. Applies
    /// only where the window can grab. On by
    /// default; files saved before it existed load with it on.
    #[serde(default = "pointer_lock_enabled")]
    pub pointer_lock: bool,
    /// Raw mouse motion → logical px while the pointer is grabbed. Raw motion
    /// is the device's, without OS acceleration, so it can feel slower or
    /// faster than the visible cursor; this evens it out. Files saved before
    /// it existed load with 1.
    #[serde(default = "unit_ratio")]
    pub pointer_lock_speed: f32,
    /// Per-gesture raw motion ratios, on top of the device calibration above.
    /// Legacy files get the same revised defaults as new documents.
    #[serde(default = "held_pan_speed")]
    pub pointer_lock_pan_speed: f32,
    #[serde(default = "held_orbit_speed")]
    pub pointer_lock_orbit_speed: f32,
}

fn held_pan_speed() -> f32 {
    1.0
}

fn held_orbit_speed() -> f32 {
    0.25
}

fn zoom_in_hold_enabled() -> bool {
    true
}

fn pointer_lock_enabled() -> bool {
    true
}

fn fov_wheel_enabled() -> bool {
    true
}

fn orbit_anchor_visible() -> bool {
    true
}

fn unit_ratio() -> f32 {
    1.0
}

impl Default for NavigationSettings {
    fn default() -> Self {
        Self::cad()
    }
}

impl NavigationSettings {
    /// A preset names its bindings; changing speed or rotation mode leaves the
    /// layout recognisable. Individual binding edits produce a custom layout.
    pub fn preset_name(self) -> &'static str {
        let bindings = |s: Self| [s.middle, s.shift_middle, s.secondary, s.space_primary];
        if bindings(self) == bindings(Self::cad()) {
            "CAD"
        } else if bindings(self) == bindings(Self::middle_orbit()) {
            "Middle orbit"
        } else {
            "Custom"
        }
    }

    pub fn cad() -> Self {
        Self {
            middle: NavigationAction::Pan,
            shift_middle: NavigationAction::Orbit,
            secondary: NavigationAction::Orbit,
            space_primary: NavigationAction::Pan,
            orbit_speed: 1.0,
            zoom_speed: 1.0,
            keep_horizon: true,
            invert_zoom: false,
            show_orbit_anchor: true,
            pan_grip: PanGrip::Pointer,
            enabled: EnabledMotion::default(),
            limits: NavigationLimits::default(),
            smoothing: SmoothingSettings::default(),
            momentum: MomentumSettings::default(),
            orbit_vertical_ratio: 1.0,
            pass_poles: false,
            zoom_in_hold: zoom_in_hold_enabled(),
            zoom_through: false,
            fov_wheel: true,
            view_cube_size: ViewCubeSize::Medium,
            pointer_lock: true,
            pointer_lock_speed: 1.0,
            pointer_lock_pan_speed: held_pan_speed(),
            pointer_lock_orbit_speed: held_orbit_speed(),
        }
    }

    pub fn middle_orbit() -> Self {
        Self {
            middle: NavigationAction::Orbit,
            shift_middle: NavigationAction::Pan,
            secondary: NavigationAction::Disabled,
            ..Self::cad()
        }
    }

    pub fn validate(self) -> Result<(), String> {
        self.limits.validate()?;
        self.smoothing.validate()?;
        self.momentum.validate()?;
        for (name, speed) in [("orbit", self.orbit_speed), ("zoom", self.zoom_speed)] {
            if !speed.is_finite() || !(0.25..=3.0).contains(&speed) {
                return Err(format!(
                    "camera {name} speed must be finite and between 0.25 and 3"
                ));
            }
        }
        if !self.orbit_vertical_ratio.is_finite()
            || !(0.25..=4.0).contains(&self.orbit_vertical_ratio)
        {
            return Err("camera vertical orbit ratio must be finite and between 0.25 and 4".into());
        }
        for speed in [
            self.pointer_lock_speed,
            self.pointer_lock_pan_speed,
            self.pointer_lock_orbit_speed,
        ] {
            if !speed.is_finite() || !(0.25..=4.0).contains(&speed) {
                return Err("grabbed pointer speed must be finite and between 0.25 and 4".into());
            }
        }
        Ok(())
    }

    pub fn configure(self, camera: CameraController) -> CameraController {
        self.configure_with_up(camera, Vec3::Z)
    }

    /// CAD uses Z-up; a Y-up engine adapter can supply its own unit axis.
    /// The shared controller validates the axis as part of its contract.
    pub fn configure_with_up(self, camera: CameraController, world_up: Vec3) -> CameraController {
        self.validate()
            .expect("navigation settings validated at the UI/file boundary");
        // Core clamps radial distance. Our limit is axial world/px; radial
        // distance is never smaller than this axial depth. Lower core's guard
        // when necessary so it cannot override a small viewport's zoom limit.
        let min_focus_distance = match camera.projection() {
            Projection::Perspective { vertical_fov } => (self.limits.min_scale.0 as f32
                * camera.viewport().y
                / (2.0 * (vertical_fov * 0.5).tan()))
            .min(0.01),
            _ => 0.01,
        };
        // `keep_horizon` is not a controller setting: world-up orbit is the
        // ground branch of `NavigationDrag`, free orbit the controller's.
        camera.with_config(CameraControllerConfig {
            world_up,
            orbit_sensitivity: self.orbit_sensitivity(),
            zoom_sensitivity: self.zoom_sensitivity(),
            min_focus_distance,
        })
    }

    pub fn zoom_sensitivity(self) -> f32 {
        0.002 * self.zoom_speed * if self.invert_zoom { -1.0 } else { 1.0 }
    }

    fn orbit_sensitivity(self) -> Vec2 {
        let horizontal = 0.005 * self.orbit_speed;
        Vec2::new(horizontal, horizontal * self.orbit_vertical_ratio)
    }

    /// Wheel/pinch zoom to `ln(target)` model units/px at `anchor`, which
    /// keeps its pixel through one exponential step. The target is clamped to
    /// the limits — in log space, so a Page event or a huge wheel delta
    /// neither overflows nor underflows. An anchor behind the camera is
    /// refused, as by [`CameraController::zoom`].
    ///
    /// `zoom_through` (perspective, zooming in below `min_scale`): the
    /// zoom first reaches the limit, then the camera travels on along the ray
    /// to the anchor, and the pivot moves ahead with it at the limit's
    /// distance `reach`. The travel continues the exponential zoom with its
    /// slope at the limit — `reach` per log unit — so it is additive over
    /// steps (smoothing and separate notches agree) and passes the anchor
    /// linearly instead of approaching it forever. A step travels at most
    /// `ln(max/min)` reaches, the zoom range's own span. An anchor already
    /// nearer than the limit (the camera is passing through) counts from its
    /// own scale, so continued zoom-in keeps going.
    pub fn zoom_to(self, camera: &mut CameraController, anchor: Vec3, log_target: f32) {
        assert!(log_target.is_finite(), "zoom target must be finite");
        if !anchor.is_finite() || camera.pose().world_to_view(anchor).z >= 0.0 {
            return;
        }
        let min = (self.limits.min_scale.0 as f32).ln();
        let max = (self.limits.max_scale.0 as f32).ln();
        let now = camera.scale_at(anchor).ln();
        let through = self.zoom_through
            && matches!(camera.projection(), Projection::Perspective { .. })
            && log_target < min
            && log_target < now;
        if !through {
            // The factor stays in log form: a zero step is exactly exp(0) = 1,
            // so a smoothed zoom's first sample (target = the current scale)
            // leaves the pose bit-identical. A linear ratio exp(target)/scale
            // is an ulp off 1 and moves the pose.
            camera.zoom_by((log_target.clamp(min, max) - now).exp(), anchor);
            return;
        }
        if now > min {
            camera.zoom_by((min - now).exp(), anchor);
        }
        let steps = (now.min(min) - log_target).min(max - min);
        let pose = camera.pose();
        let offset = anchor - pose.position;
        // Scale is proportional to axial depth, and so to distance along
        // this ray: `reach` is where the ray's scale equals the limit.
        let reach = offset.length() * (min - camera.scale_at(anchor).ln()).exp();
        let direction = offset / offset.length();
        let position = pose.position + direction * (reach * steps);
        camera.set_pose(
            CameraPose {
                position,
                orientation: pose.orientation,
            },
            position + direction * reach,
        );
    }

    pub fn begin_drag(
        self,
        camera: CameraController,
        start: ScreenPoint,
        anchor: Vec3,
        kind: GestureKind,
    ) -> NavigationDrag {
        self.begin_drag_with_up(camera, start, anchor, kind, Vec3::Z)
    }

    /// Orbit attitude is independent of the picked point's position on screen.
    /// Store the ground frame once, alongside the immutable press snapshot.
    pub fn begin_drag_with_up(
        self,
        camera: CameraController,
        start: ScreenPoint,
        anchor: Vec3,
        kind: GestureKind,
        world_up: Vec3,
    ) -> NavigationDrag {
        let camera = self.configure_with_up(camera, world_up);
        let mut drag = NavigationDrag::new(camera, start, anchor, kind, self.pan_grip);
        // The hold's zoom is clamped to the limits relative to the press
        // scale; a press already beyond a limit may only zoom back towards it.
        let scale = camera.scale_at(anchor);
        drag.zoom_range = [
            (self.limits.min_scale.0 as f32 / scale).ln().min(0.0),
            (self.limits.max_scale.0 as f32 / scale).ln().max(0.0),
        ];
        if kind == GestureKind::Orbit && self.keep_horizon {
            let back = -camera.pose().forward();
            let height = back.dot(world_up);
            let horizontal = back - world_up * height;
            let right = world_up.cross(back);
            // At an exact top/bottom view azimuth is undefined. The saved
            // screen-right supplies it, so acquiring a gesture never jumps.
            let screen_right = camera.pose().orientation * Vec3::X;
            let right = if right.length_squared() > 1e-10 {
                right.normalize()
            } else {
                (screen_right - world_up * screen_right.dot(world_up)).normalize()
            };
            // Past a pole the camera is upside down and its screen right
            // opposes `up × back`. Tilting about the screen's side keeps a
            // vertical drag turning the view the way it did before the pole.
            let right = if self.pass_poles && right.dot(screen_right) < 0.0 {
                -right
            } else {
                right
            };
            drag.ground = Some(GroundOrbit {
                up: world_up,
                right,
                // atan2 retains the tiny horizontal component near a pole;
                // asin(height) rounds to 90° early in f32 and can cross it.
                elevation: height.atan2(horizontal.length()),
                sensitivity: self.orbit_sensitivity(),
                pass_poles: self.pass_poles,
            });
        }
        drag
    }

    pub fn permitted(self, action: NavigationAction) -> NavigationAction {
        match action {
            NavigationAction::Pan if self.enabled.pan => action,
            NavigationAction::Orbit if self.enabled.orbit => action,
            _ => NavigationAction::Disabled,
        }
    }
}

/// Immutable press snapshot: sampling the same total displacement yields the
/// same camera, independently of event frequency or the render frame rate.
/// Inertia after release ([`crate::momentum::Momentum`]) keeps sampling it.
///
/// Zoom inside the hold is a separate total, not a pointer event: the
/// snapshot zoomed about the anchor by `zoom`, then moved by the pointer. A
/// wheel step makes a new value ([`Self::zoomed`]), so the camera depends
/// only on the two totals, not on how wheel and pointer events interleave.
#[derive(Clone, Copy, Debug)]
pub struct NavigationDrag {
    start: ScreenPoint,
    camera: CameraController,
    anchor: Vec3,
    kind: GestureKind,
    ground: Option<GroundOrbit>,
    pan_grip: PanGrip,
    /// `ln(scale at the anchor now / at the press)`; 0 without hold zoom.
    zoom: f32,
    /// Bounds of `zoom` from the limits, see `begin_drag_with_up`.
    zoom_range: [f32; 2],
}

/// An explicit world-up policy over the shared pose/controller. The core's
/// turntable aims at its anchor and rejects whole pitch steps near a pole;
/// a CAD off-centre gesture instead constrains the camera's viewing direction.
#[derive(Clone, Copy, Debug)]
struct GroundOrbit {
    up: Vec3,
    right: Vec3,
    elevation: f32,
    sensitivity: Vec2,
    pass_poles: bool,
}

impl NavigationDrag {
    fn new(
        camera: CameraController,
        start: ScreenPoint,
        anchor: Vec3,
        kind: GestureKind,
        pan_grip: PanGrip,
    ) -> Self {
        assert!(start.0.is_finite(), "navigation press must be finite");
        assert!(
            anchor.is_finite() && camera.pose().world_to_view(anchor).z < 0.0,
            "navigation anchor must be finite and in front of the camera"
        );
        Self {
            start,
            camera,
            anchor,
            kind,
            ground: None,
            pan_grip,
            zoom: 0.0,
            zoom_range: [0.0, 0.0],
        }
    }

    pub fn sample(self, point: ScreenPoint) -> CameraController {
        assert!(point.0.is_finite(), "navigation pointer must be finite");
        // The press takes hold of the anchor: it is the pivot from the first
        // sample on. By default release means stop at the pointed view; the
        // optional inertia samples this same snapshot at a virtual pointer.
        // The hold's zoom applies to the snapshot first. Zoom about the
        // anchor keeps its pixel, so the gesture below sees a press camera
        // that differs only in scale, and keeps its anchor relation.
        let mut press = self.camera;
        if self.zoom != 0.0 {
            press.zoom_by(self.zoom.exp(), self.anchor);
        }
        let mut camera = press;
        camera.set_pose(camera.pose(), self.anchor);
        let delta = point.0 - self.start.0;
        match (self.kind, self.ground) {
            (GestureKind::Orbit, Some(ground)) => {
                if delta != Vec2::ZERO {
                    let yaw = Quat::from_axis_angle(ground.up, -delta.x * ground.sensitivity.x);
                    let tilt = delta.y * ground.sensitivity.y;
                    // Rotation about the (yawed) side axis; positive raises the
                    // camera. Without `pass_poles` elevation stops at the poles.
                    let raise = if ground.pass_poles {
                        tilt
                    } else {
                        let limit = std::f32::consts::FRAC_PI_2;
                        (ground.elevation + tilt).clamp(-limit, limit) - ground.elevation
                    };
                    let pitch = Quat::from_axis_angle(yaw * ground.right, -raise);
                    let rotation = pitch * yaw;
                    let mut pose = press.pose();
                    pose.position = self.anchor + rotation * (pose.position - self.anchor);
                    // Applying the same ground rotation to the frame preserves a
                    // saved roll continuously; it cannot accumulate new roll over
                    // successive gestures. At poles the retained right avoids a
                    // singular look_at reconstruction and an abrupt 180° flip.
                    pose.orientation = (rotation * pose.orientation).normalize();
                    camera.set_pose(pose, self.anchor);
                }
            }
            (GestureKind::Orbit, None) => camera.orbit_free(self.anchor, delta),
            (GestureKind::Pan, _) => {
                // The core pan measures world units per pixel at the anchor's depth,
                // so whatever the host passed as anchor follows the pointer exactly.
                // It then leaves the pivot at `anchor + translation`: the same spot on
                // screen, which is the view centre for `PivotDepth` (anchor = pivot).
                camera.pan(self.anchor, delta);
                if self.pan_grip == PanGrip::Pointer {
                    // The grabbed point itself becomes the pivot: it is what the user
                    // is holding, and what a following zoom/orbit should refer to.
                    camera.set_pose(camera.pose(), self.anchor);
                }
            }
        }
        if self.kind == GestureKind::Orbit && point != self.start {
            // Keep the anchor's original view coordinates (including depth).
            // Both policies rotate the saved frame and offset together; this
            // correction bounds f32 drift without accumulating it across frames.
            let old = press.pose().world_to_view(self.anchor);
            let new = camera.pose().world_to_view(self.anchor);
            let mut pose = camera.pose();
            pose.position += pose.orientation * (new - old);
            camera.set_pose(pose, self.anchor);
        }
        camera
    }

    /// One wheel/pinch step inside the hold, `ln` of the scale ratio
    /// (< 0 zooms in), added to the hold's total and clamped to the limits.
    pub fn zoomed(self, log_step: f32) -> Self {
        assert!(log_step.is_finite(), "hold zoom step must be finite");
        self.with_zoom(self.zoom + log_step)
    }

    /// Replace the hold's log-scale bounds with limits supplied by a host.
    /// Zero must remain inside the range: acquiring a hold cannot change its scale.
    pub fn with_zoom_range(mut self, range: [f32; 2]) -> Self {
        assert!(
            range[0].is_finite() && range[1].is_finite() && range[0] <= 0.0 && range[1] >= 0.0,
            "hold zoom range must be finite and include zero"
        );
        self.zoom_range = range;
        self.zoom = self.zoom.clamp(range[0], range[1]);
        self
    }

    /// The same snapshot with the hold's total zoom replaced (clamped).
    pub fn with_zoom(mut self, zoom: f32) -> Self {
        assert!(zoom.is_finite(), "hold zoom must be finite");
        self.zoom = zoom.clamp(self.zoom_range[0], self.zoom_range[1]);
        self
    }

    /// `ln(scale at the anchor now / at the press)`, 0 unless zoomed in hold.
    pub fn zoom(self) -> f32 {
        self.zoom
    }

    pub fn anchor(self) -> Vec3 {
        self.anchor
    }
    /// Pointer at the press. A chord inside a hold starts a new gesture at the
    /// pointer of the switch, so (start, anchor, kind) identify a segment.
    pub fn start(self) -> ScreenPoint {
        self.start
    }
    pub fn kind(self) -> GestureKind {
        self.kind
    }
}
