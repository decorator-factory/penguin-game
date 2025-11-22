use std::borrow::Cow;

use macroquad::prelude as mq;

#[derive(enumset::EnumSetType, Debug)]
pub enum Input {
    Left,
    Right,
    Shoot,
}

pub trait InputDevice {
    fn next_frame(&mut self);
    fn is_input_down(&self, input: Input) -> bool;
    fn look_angle_radians(&self) -> f32;

    fn device_info(&'_ self) -> Cow<'_, str>;
}

#[derive(Clone, Copy, Debug)]
pub struct MacroquadInput;

impl InputDevice for MacroquadInput {
    fn next_frame(&mut self) {}

    fn is_input_down(&self, input: Input) -> bool {
        match input {
            Input::Left => mq::is_key_down(mq::KeyCode::A),
            Input::Right => mq::is_key_down(mq::KeyCode::D),
            Input::Shoot => mq::is_mouse_button_down(mq::MouseButton::Left),
        }
    }

    fn look_angle_radians(&self) -> f32 {
        let (sx, sy) = miniquad::window::screen_size();
        let (mx, my) = mq::mouse_position();

        mq::vec2(mx - sx / 2., my - sy / 2.).to_angle()
    }

    fn device_info(&'_ self) -> Cow<'_, str> {
        Cow::Borrowed("macroquad")
    }
}

/// [`InputDevice`] used to play back a demo movie and then record
pub struct ComposedInput<A, B> {
    first: A,
    second: B,
    names: (Cow<'static, str>, Cow<'static, str>),
    threshold_frame: u64,
    current_frame: u64,
}

impl<A, B> ComposedInput<A, B> {
    /// `threshold_frame` is the *first frame* on which (and after which)
    /// the second input device will be used.
    pub fn new(
        first: A,
        second: B,
        threshold_frame: u64,
        names: (Cow<'static, str>, Cow<'static, str>),
    ) -> ComposedInput<A, B> {
        ComposedInput { first, second, threshold_frame, names, current_frame: 0 }
    }

    pub fn into_inner(self) -> (A, B) {
        (self.first, self.second)
    }
}

impl<A, B> InputDevice for ComposedInput<A, B>
where
    A: InputDevice,
    B: InputDevice,
{
    fn next_frame(&mut self) {
        if self.current_frame >= self.threshold_frame {
            self.second.next_frame();
        } else {
            self.first.next_frame();
        }
        self.current_frame += 1;
    }

    fn is_input_down(&self, input: Input) -> bool {
        if self.current_frame >= self.threshold_frame {
            self.second.is_input_down(input)
        } else {
            self.first.is_input_down(input)
        }
    }

    fn look_angle_radians(&self) -> f32 {
        if self.current_frame >= self.threshold_frame {
            self.second.look_angle_radians()
        } else {
            self.first.look_angle_radians()
        }
    }

    fn device_info(&'_ self) -> Cow<'_, str> {
        Cow::Owned(if self.current_frame >= self.threshold_frame {
            format!("{} ({})", self.names.1, self.second.device_info())
        } else {
            let diff = self.threshold_frame - self.current_frame;
            if diff <= 1200 {
                format!(
                    "{} ({}) [switching in {diff} frames]",
                    self.names.0,
                    self.first.device_info()
                )
            } else {
                format!("{} ({})", self.names.0, self.first.device_info())
            }
        })
    }
}
