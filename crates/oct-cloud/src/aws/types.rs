/// Represents an AWS instance type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceType {
    /// The name of the instance type.
    pub name: &'static str,

    /// The number of CPUs for the instance type.
    pub cpus: usize,
    /// The amount of memory (in MB) for the instance type.
    pub memory: usize,
}

impl InstanceType {
    /// Represents the `t2.micro` instance type.
    pub const T2_MICRO: Self = Self {
        name: "t2.micro",

        cpus: 1,
        memory: 1024,
    };
}

impl From<&str> for InstanceType {
    /// Creates an `InstanceType` from a string.
    ///
    /// # Panics
    ///
    /// Panics if the string is not a valid instance type.
    fn from(value: &str) -> Self {
        match value {
            "t2.micro" => Self::T2_MICRO,
            _ => panic!("Invalid instance type: {value}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::InstanceType;

    #[test]
    fn test_from_str() {
        assert_eq!(InstanceType::from("t2.micro"), InstanceType::T2_MICRO);
    }

    #[test]
    #[should_panic(expected = "Invalid instance type: invalid")]
    fn test_from_str_invalid() {
        let _ = InstanceType::from("invalid");
    }
}
