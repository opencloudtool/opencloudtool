use aws_sdk_route53::types::RrType;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents an AWS resource record type.
#[derive(Debug, Clone, PartialEq, Eq, Copy, Serialize, Deserialize)]
pub enum RecordType {
    A,
    NS,
    SOA,
    TXT,
}

impl From<&str> for RecordType {
    fn from(s: &str) -> Self {
        match s {
            "A" => Self::A,
            "NS" => Self::NS,
            "SOA" => Self::SOA,
            "TXT" => Self::TXT,
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
            RrType::Txt => Self::TXT,
            _ => panic!("Invalid record type: {rr_type}"),
        }
    }
}

impl From<RecordType> for RrType {
    fn from(value: RecordType) -> Self {
        match value {
            RecordType::A => Self::A,
            RecordType::NS => Self::Ns,
            RecordType::SOA => Self::Soa,
            RecordType::TXT => Self::Txt,
        }
    }
}

impl RecordType {
    pub fn as_str(&self) -> &str {
        match self {
            RecordType::A => "A",
            RecordType::NS => "NS",
            RecordType::SOA => "SOA",
            RecordType::TXT => "TXT",
        }
    }
}

impl fmt::Display for RecordType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Represents an AWS instance type.
#[derive(Debug, PartialEq, Eq)]
pub struct InstanceInfo {
    /// The number of CPUs for the instance type.
    pub cpus: u32,
    /// The amount of memory (in MB) for the instance type.
    pub memory: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstanceType {
    T2Micro,
    T3Medium,
}

impl InstanceType {
    pub fn as_str(&self) -> &str {
        match self {
            InstanceType::T2Micro => "t2.micro",
            InstanceType::T3Medium => "t3.medium",
        }
    }

    pub fn get_info(&self) -> InstanceInfo {
        match self {
            InstanceType::T2Micro => InstanceInfo {
                cpus: 1000,
                memory: 1024,
            },
            InstanceType::T3Medium => InstanceInfo {
                cpus: 2000,
                memory: 4096,
            },
        }
    }
}

impl From<&str> for InstanceType {
    /// Creates an `InstanceType` from a string.
    ///
    /// # Panics
    ///
    /// Panics if the string is not a valid instance type.
    fn from(value: &str) -> Self {
        match value {
            "t2.micro" => InstanceType::T2Micro,
            "t3.medium" => InstanceType::T3Medium,
            _ => panic!("Invalid instance type: {value}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use aws_sdk_route53::types::RrType;

    use super::*;

    #[test]
    fn test_display() {
        assert_eq!(RecordType::A.to_string(), "A");
        assert_eq!(RecordType::NS.to_string(), "NS");
        assert_eq!(RecordType::SOA.to_string(), "SOA");
        assert_eq!(RecordType::TXT.to_string(), "TXT");
    }

    #[test]
    fn test_rr_type_from_record_type() {
        assert_eq!(RrType::from(RecordType::A), RrType::A);
        assert_eq!(RrType::from(RecordType::NS), RrType::Ns);
        assert_eq!(RrType::from(RecordType::SOA), RrType::Soa);
        assert_eq!(RrType::from(RecordType::TXT), RrType::Txt);
    }

    #[test]
    fn test_record_type_from_str() {
        assert_eq!(RecordType::from("A"), RecordType::A);
        assert_eq!(RecordType::from("NS"), RecordType::NS);
        assert_eq!(RecordType::from("SOA"), RecordType::SOA);
        assert_eq!(RecordType::from("TXT"), RecordType::TXT);
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
        assert_eq!(
            RecordType::from(aws_sdk_route53::types::RrType::Ns),
            RecordType::NS
        );
        assert_eq!(
            RecordType::from(aws_sdk_route53::types::RrType::Soa),
            RecordType::SOA
        );
        assert_eq!(
            RecordType::from(aws_sdk_route53::types::RrType::Txt),
            RecordType::TXT
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
        assert_eq!(RecordType::NS.as_str(), "NS");
        assert_eq!(RecordType::SOA.as_str(), "SOA");
        assert_eq!(RecordType::TXT.as_str(), "TXT");
    }

    #[test]
    fn test_instance_type_as_str() {
        assert_eq!(InstanceType::T2Micro.as_str(), "t2.micro");
        assert_eq!(InstanceType::T3Medium.as_str(), "t3.medium");
    }

    #[test]
    fn test_instance_type_get_info() {
        assert_eq!(
            InstanceType::T2Micro.get_info(),
            InstanceInfo {
                cpus: 1000,
                memory: 1024
            }
        );
        assert_eq!(
            InstanceType::T3Medium.get_info(),
            InstanceInfo {
                cpus: 2000,
                memory: 4096
            }
        );
    }

    #[test]
    fn test_instance_type_from_str() {
        assert_eq!(InstanceType::from("t2.micro"), InstanceType::T2Micro);
        assert_eq!(InstanceType::from("t3.medium"), InstanceType::T3Medium);
    }

    #[test]
    #[should_panic(expected = "Invalid instance type: invalid")]
    fn test_instance_type_from_str_invalid() {
        let _ = InstanceType::from("invalid");
    }
}
