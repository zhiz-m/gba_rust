#![allow(non_camel_case_types)]

pub enum Cp15Register {
    C0_C0_0 = 0,
    C0_C0_1 = 1,
    C0_C0_2 = 2,
    C1_C0_0 = 3,
    C2_C0_0 = 4,
    C2_C0_1 = 5,
    C3_C0_0 = 6,
    C5_C0_0 = 7,
    C5_C0_1 = 8,
    C5_C0_2 = 9,
    C5_C0_3 = 10,

    C6_C0_0 = 11,
    C6_C1_0 = 12,
    C6_C2_0 = 13,
    C6_C3_0 = 14,
    C6_C4_0 = 15,
    C6_C5_0 = 16,
    C6_C6_0 = 17,
    C6_C7_0 = 18,

    C6_C0_1 = 19,
    C6_C1_1 = 20,
    C6_C2_1 = 21,
    C6_C3_1 = 22,
    C6_C4_1 = 23,
    C6_C5_1 = 24,
    C6_C6_1 = 25,
    C6_C7_1 = 26,

    C7_CM_XX = 27,
    C9_C0_0 = 28,
    C9_C0_1 = 29,
    C9_C1_0 = 30,
    C9_C1_1 = 31,
}

#[derive(Default)]
pub struct Cp15 {
    pub reg: [u32; 32],
}

impl Cp15 {
    pub fn tuple_to_cp15_reg(tuple: (u32, u32, u32)) -> Option<Cp15Register> {
        Some(match tuple {
            (0, 0, 0) => Cp15Register::C0_C0_0,
            (0, 0, 1) => Cp15Register::C0_C0_1,
            (0, 0, 2) => Cp15Register::C0_C0_2,

            (1, 0, 0) => Cp15Register::C1_C0_0,
            (2, 0, 0) => Cp15Register::C2_C0_0,
            (2, 0, 1) => Cp15Register::C2_C0_1,
            (3, 0, 0) => Cp15Register::C3_C0_0,
            (5, 0, 0) => Cp15Register::C5_C0_0,
            (5, 0, 1) => Cp15Register::C5_C0_1,
            (5, 0, 2) => Cp15Register::C5_C0_2,
            (5, 0, 3) => Cp15Register::C5_C0_3,

            (6, 0, 0) => Cp15Register::C6_C0_0,
            (6, 1, 0) => Cp15Register::C6_C1_0,
            (6, 2, 0) => Cp15Register::C6_C2_0,
            (6, 3, 0) => Cp15Register::C6_C3_0,
            (6, 4, 0) => Cp15Register::C6_C4_0,
            (6, 5, 0) => Cp15Register::C6_C5_0,
            (6, 6, 0) => Cp15Register::C6_C6_0,
            (6, 7, 0) => Cp15Register::C6_C7_0,

            (6, 0, 1) => Cp15Register::C6_C0_1,
            (6, 1, 1) => Cp15Register::C6_C1_1,
            (6, 2, 1) => Cp15Register::C6_C2_1,
            (6, 3, 1) => Cp15Register::C6_C3_1,
            (6, 4, 1) => Cp15Register::C6_C4_1,
            (6, 5, 1) => Cp15Register::C6_C5_1,
            (6, 6, 1) => Cp15Register::C6_C6_1,
            (6, 7, 1) => Cp15Register::C6_C7_1,

            (7, _, _) => Cp15Register::C7_CM_XX,

            (9, 0, 0) => Cp15Register::C9_C0_0,
            (9, 0, 1) => Cp15Register::C9_C0_1,
            (9, 1, 0) => Cp15Register::C9_C1_0,
            (9, 1, 1) => Cp15Register::C9_C1_1,

            _ => return None
        })
    }

    pub fn get_dtcm_addr(val: u32) -> u32{
        val & !0xfff
    }
}
