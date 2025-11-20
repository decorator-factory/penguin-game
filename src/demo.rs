use crate::input::{
    Input,
    InputDevice,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DemoAction {
    InputOn(Input),
    InputOff(Input),
    SetLookAngle(f32), // degrees!
}

pub struct DemoInput {
    frame: u64,
    actions: Box<[(u64, DemoAction)]>, // should be sorted by u64
    action_index: usize,
    look_angle: f32,
    // TODO: use enumset or something like that
    is_left_on: bool,
    is_right_on: bool,
    is_shoot_on: bool,
}

impl DemoInput {
    pub fn new(actions: Box<[(u64, DemoAction)]>) -> DemoInput {
        {
            // ensure frame numbers are non-decreasing
            let mut last_frame = 0u64;
            for (i, (frame, action)) in actions.iter().enumerate() {
                assert!(*frame >= last_frame, "Instruction out of place: {:?}", (i, frame, action));
                last_frame = *frame;
            }
        }

        DemoInput {
            actions,
            frame: 0,
            action_index: 0,
            look_angle: 0.0,
            is_left_on: false,
            is_right_on: false,
            is_shoot_on: false,
        }
    }

    fn handle_action(&mut self, action: DemoAction) {
        match action {
            DemoAction::InputOn(input) => match input {
                Input::Left => self.is_left_on = true,
                Input::Right => self.is_right_on = true,
                Input::Shoot => self.is_shoot_on = true,
            },
            DemoAction::InputOff(input) => match input {
                Input::Left => self.is_left_on = false,
                Input::Right => self.is_right_on = false,
                Input::Shoot => self.is_shoot_on = false,
            },
            DemoAction::SetLookAngle(angle) => self.look_angle = angle.to_radians(),
        }
    }
}

impl InputDevice for DemoInput {
    fn is_input_down(&self, input: Input) -> bool {
        match input {
            Input::Left => self.is_left_on,
            Input::Right => self.is_right_on,
            Input::Shoot => self.is_shoot_on,
        }
    }

    fn look_angle(&self) -> f32 {
        self.look_angle
    }

    fn next_frame(&mut self) {
        self.frame += 1;

        if self.action_index >= self.actions.len() {
            return;
        }

        loop {
            let Some((frame, action)) = self.actions.get(self.action_index) else { return };
            if *frame == self.frame {
                self.handle_action(*action);
                self.action_index += 1;
            } else {
                return;
            }
        }
    }
}

pub static DEMO_MOVIE: &[(u64, DemoAction)] = &[
    //
    // Get on top of the house
    (150, DemoAction::SetLookAngle(30.0)),
    (200, DemoAction::SetLookAngle(60.0)),
    (250, DemoAction::SetLookAngle(75.0)),
    (300, DemoAction::SetLookAngle(90.0)),
    (300, DemoAction::InputOn(Input::Shoot)),
    (304, DemoAction::InputOff(Input::Shoot)),
    (316, DemoAction::InputOn(Input::Left)),
    (332, DemoAction::InputOff(Input::Left)),
    (336, DemoAction::InputOn(Input::Right)),
    (344, DemoAction::InputOff(Input::Right)),
    (600, DemoAction::InputOn(Input::Shoot)),
    (604, DemoAction::InputOff(Input::Shoot)),
    (604, DemoAction::InputOn(Input::Right)),
    (612, DemoAction::InputOff(Input::Right)),
    (660, DemoAction::InputOn(Input::Left)),
    (792, DemoAction::InputOff(Input::Left)),
    //
    // Get onto the first bridge
    (800, DemoAction::SetLookAngle(100.0)),
    (820, DemoAction::SetLookAngle(110.0)),
    (840, DemoAction::SetLookAngle(120.0)),
    (860, DemoAction::InputOn(Input::Right)),
    (872, DemoAction::InputOn(Input::Shoot)),
    (876, DemoAction::InputOff(Input::Shoot)),
    (1000, DemoAction::InputOff(Input::Right)),
    (1000, DemoAction::SetLookAngle(110.0)),
    (1020, DemoAction::InputOn(Input::Left)),
    (1020, DemoAction::SetLookAngle(100.0)),
    (1040, DemoAction::SetLookAngle(95.0)),
    (1060, DemoAction::SetLookAngle(90.0)),
    (1080, DemoAction::InputOff(Input::Left)),
    //
    // Get onto the second bridge
    (1100, DemoAction::InputOn(Input::Right)),
    (1300, DemoAction::InputOn(Input::Shoot)),
    (1304, DemoAction::InputOff(Input::Shoot)),
    (1464, DemoAction::InputOff(Input::Right)),
    (1464, DemoAction::InputOn(Input::Left)),
    (1520, DemoAction::InputOff(Input::Left)),
    //
    // Get onto the third bridge
    (1540, DemoAction::SetLookAngle(95.0)),
    (1552, DemoAction::SetLookAngle(100.0)),
    (1564, DemoAction::InputOn(Input::Right)),
    (1700, DemoAction::SetLookAngle(110.0)),
    (1740, DemoAction::SetLookAngle(120.0)),
    (1900, DemoAction::InputOn(Input::Shoot)),
    (1904, DemoAction::InputOff(Input::Shoot)),
    (2208, DemoAction::InputOff(Input::Right)),
    //
    // Get onto the fourth bridge
    (2216, DemoAction::SetLookAngle(110.0)),
    (2232, DemoAction::SetLookAngle(100.0)),
    (2300, DemoAction::InputOn(Input::Right)),
    (2316, DemoAction::SetLookAngle(90.0)),
    (2440, DemoAction::InputOn(Input::Shoot)),
    (2444, DemoAction::InputOff(Input::Shoot)),
    (2600, DemoAction::InputOff(Input::Right)),
    (2700, DemoAction::InputOn(Input::Left)),
    (2752, DemoAction::InputOff(Input::Left)),
    //
    // Get into the tower
    (2780, DemoAction::SetLookAngle(85.0)),
    (2800, DemoAction::SetLookAngle(80.0)),
    (2800, DemoAction::InputOn(Input::Right)),
    (2832, DemoAction::InputOff(Input::Right)),
    (2840, DemoAction::InputOn(Input::Left)),
    (2900, DemoAction::InputOn(Input::Shoot)),
    (2904, DemoAction::InputOff(Input::Shoot)),
    (2924, DemoAction::SetLookAngle(85.0)),
    (2940, DemoAction::SetLookAngle(90.0)),
    (3200, DemoAction::InputOff(Input::Left)),
    //
    // Sync onto the crazy ledge
    (3240, DemoAction::InputOn(Input::Shoot)),
    (3244, DemoAction::InputOff(Input::Shoot)),
    (3448, DemoAction::InputOn(Input::Shoot)),
    (3452, DemoAction::InputOff(Input::Shoot)),
    (3584, DemoAction::InputOn(Input::Shoot)),
    (3588, DemoAction::InputOff(Input::Shoot)),
    (3700, DemoAction::InputOn(Input::Right)),
    (3872, DemoAction::InputOff(Input::Right)),
    (3916, DemoAction::InputOn(Input::Left)),
    (3924, DemoAction::InputOff(Input::Left)),
    //
    // Jump onto the tiny leg
    (3952, DemoAction::SetLookAngle(95.0)),
    (3956, DemoAction::InputOn(Input::Right)),
    (3980, DemoAction::InputOff(Input::Right)),
    (4000, DemoAction::InputOn(Input::Shoot)),
    (4004, DemoAction::InputOff(Input::Shoot)),
    (4120, DemoAction::InputOn(Input::Left)),
    (4164, DemoAction::InputOff(Input::Left)),
    //
    // Jump onto the finish ledge
    (4200, DemoAction::SetLookAngle(90.0)),
    (4240, DemoAction::SetLookAngle(85.0)),
    (4320, DemoAction::InputOn(Input::Shoot)),
    (4324, DemoAction::InputOff(Input::Shoot)),
    (4340, DemoAction::InputOn(Input::Left)),
    (4444, DemoAction::InputOff(Input::Left)),
    (4452, DemoAction::InputOn(Input::Right)),
    (4500, DemoAction::InputOff(Input::Right)),
    //
    // Victory spin
    (4540, DemoAction::SetLookAngle(80.0)),
    (4552, DemoAction::SetLookAngle(100.0)),
    (4564, DemoAction::SetLookAngle(120.0)),
    (4576, DemoAction::SetLookAngle(140.0)),
    (4588, DemoAction::SetLookAngle(160.0)),
    (4600, DemoAction::SetLookAngle(180.0)),
    (4612, DemoAction::SetLookAngle(200.0)),
    (4624, DemoAction::SetLookAngle(220.0)),
    (4636, DemoAction::SetLookAngle(240.0)),
    (4648, DemoAction::SetLookAngle(260.0)),
    (4660, DemoAction::SetLookAngle(280.0)),
    (4672, DemoAction::SetLookAngle(300.0)),
    (4684, DemoAction::SetLookAngle(320.0)),
    (4696, DemoAction::SetLookAngle(340.0)),
    (4708, DemoAction::SetLookAngle(0.0)),
    (4720, DemoAction::SetLookAngle(20.0)),
    (4732, DemoAction::SetLookAngle(40.0)),
    (4744, DemoAction::SetLookAngle(60.0)),
    (4756, DemoAction::SetLookAngle(80.0)),
];
