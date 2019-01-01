pub trait UUID {
    fn display_uuid(&self) -> String;
}

impl UUID for [u8; 16] {
    fn display_uuid(&self) -> String {
        let mut digits: Vec<_> = self.iter().collect();
        let mut uuid: Vec<String> = Vec::new();
        uuid.extend(digits.drain(..4).rev().map(|x| format!("{:02X}", x)));
        uuid.push("-".to_string());
        uuid.extend(digits.drain(..2).rev().map(|x| format!("{:02X}", x)));
        uuid.push("-".to_string());
        uuid.extend(digits.drain(..2).rev().map(|x| format!("{:02X}", x)));
        uuid.push("-".to_string());
        uuid.extend(digits.drain(..2).map(|x| format!("{:02X}", x)));
        uuid.push("-".to_string());
        uuid.extend(digits.drain(..).map(|x| format!("{:02X}", x)));

        uuid.into_iter().collect()
    }
}
