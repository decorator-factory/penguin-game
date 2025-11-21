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
    fn look_angle(&self) -> f32;
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

    fn look_angle(&self) -> f32 {
        let (sx, sy) = miniquad::window::screen_size();
        let (mx, my) = mq::mouse_position();

        mq::vec2(mx - sx / 2., my - sy / 2.).to_angle()
    }
}
