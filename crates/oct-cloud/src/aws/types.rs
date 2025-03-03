use aws_sdk_route53::types::RrType;

/// Represents an AWS resource record type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordType {
    A,
    NS,
    SOA,
}

impl From<&str> for RecordType {
    fn from(s: &str) -> Self {
        match s {
            "A" => Self::A,
            "NS" => Self::NS,
            "SOA" => Self::SOA,
            _ => panic!("Invalid record type: {s}"),
        }
    }
}

impl From<RrType> for RecordType {
    fn from(rr_type: RrType) -> Self {
        match rr_type {
            RrType::A => Self::A,
            RrType::Ns => Self::NS,
            RrType::Soa => Self::SOA,
            _ => panic!("Invalid record type: {rr_type}"),
        }
    }
}

impl RecordType {
    pub fn as_str(&self) -> &str {
        match self {
            RecordType::A => "A",
            RecordType::NS => "NS",
            RecordType::SOA => "SOA",
        }
    }
}
/// Represents an AWS instance type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceType {
    /// The name of the instance type.
    pub name: &'static str,

    /// The number of CPUs for the instance type.
    pub cpus: u32,
    /// The amount of memory (in MB) for the instance type.
    pub memory: u64,
}

impl InstanceType {
    /// Represents the `t2.micro` instance type.
    pub const T2_MICRO: Self = Self {
        name: "t2.micro",

        cpus: 1000,
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
    use super::{InstanceType, RecordType};

    #[test]
    fn test_record_type_from_str() {
        assert_eq!(RecordType::from("A"), RecordType::A);
    }

    #[test]
    #[should_panic(expected = "Invalid record type: invalid")]
    fn test_record_type_from_str_invalid() {
        let _ = RecordType::from("invalid");
    }

    #[test]
    fn test_record_type_from_rr_type() {
        assert_eq!(
            RecordType::from(aws_sdk_route53::types::RrType::A),
            RecordType::A
        );
    }
    #[test]
    #[should_panic(expected = "Invalid record type: AAAA")]
    fn test_record_type_from_rr_type_invalid() {
        let _ = RecordType::from(aws_sdk_route53::types::RrType::Aaaa);
    }

    #[test]
    fn test_record_type_as_str() {
        assert_eq!(RecordType::A.as_str(), "A");
    }

    #[test]
    fn test_instance_type_from_str() {
        assert_eq!(InstanceType::from("t2.micro"), InstanceType::T2_MICRO);
    }

    #[test]
    #[should_panic(expected = "Invalid instance type: invalid")]
    fn test_instance_type_from_str_invalid() {
        let _ = InstanceType::from("invalid");
    }
}
