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

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstanceType {
    T3Nano,
    T3Micro,
    T3Small,
    T3Medium,
    T3Large,
    T3Xlarge,
    T32xlarge,
}

impl InstanceType {
    pub fn as_str(&self) -> &str {
        match self {
            InstanceType::T3Nano => "t3.nano",
            InstanceType::T3Micro => "t3.micro",
            InstanceType::T3Small => "t3.small",
            InstanceType::T3Medium => "t3.medium",
            InstanceType::T3Large => "t3.large",
            InstanceType::T3Xlarge => "t3.xlarge",
            InstanceType::T32xlarge => "t3.2xlarge",
        }
    }

    /// Tries to get the smallest possible instance type for to fit requested resources
    // NOTE: The instances list must be sorted by size from smallest to largest
    pub fn from_resources(cpus: u32, memory: u64) -> Option<Self> {
        let instances = [
            Self::T3Nano,
            Self::T3Micro,
            Self::T3Small,
            Self::T3Medium,
            Self::T3Large,
            Self::T3Xlarge,
            Self::T32xlarge,
        ];

        for instance in instances {
            let info = instance.get_info();
            if cpus <= info.cpus && memory <= info.memory {
                return Some(instance);
            }
        }

        None
    }

    pub fn get_info(&self) -> InstanceInfo {
        match self {
            Self::T3Nano => InstanceInfo {
                cpus: 2000,
                memory: 512,
            },
            Self::T3Micro => InstanceInfo {
                cpus: 2000,
                memory: 1024,
            },
            Self::T3Small => InstanceInfo {
                cpus: 2000,
                memory: 2048,
            },
            Self::T3Medium => InstanceInfo {
                cpus: 2000,
                memory: 4096,
            },
            Self::T3Large => InstanceInfo {
                cpus: 2000,
                memory: 8192,
            },
            Self::T3Xlarge => InstanceInfo {
                cpus: 4000,
                memory: 16384,
            },
            Self::T32xlarge => InstanceInfo {
                cpus: 8000,
                memory: 32768,
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
            "t3.nano" => Self::T3Nano,
            "t3.micro" => Self::T3Micro,
            "t3.small" => Self::T3Small,
            "t3.medium" => Self::T3Medium,
            "t3.large" => Self::T3Large,
            "t3.xlarge" => Self::T3Xlarge,
            "t3.2xlarge" => Self::T32xlarge,
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
        assert_eq!(InstanceType::T3Nano.as_str(), "t3.nano");
        assert_eq!(InstanceType::T32xlarge.as_str(), "t3.2xlarge");
    }

    #[test]
    fn test_instance_type_get_info() {
        assert_eq!(
            InstanceType::T3Nano.get_info(),
            InstanceInfo {
                cpus: 2000,
                memory: 512
            }
        );
        assert_eq!(
            InstanceType::T32xlarge.get_info(),
            InstanceInfo {
                cpus: 8000,
                memory: 32768
            }
        );
    }

    #[test]
    fn test_instance_type_from_str() {
        assert_eq!(InstanceType::from("t3.nano"), InstanceType::T3Nano);
        assert_eq!(InstanceType::from("t3.2xlarge"), InstanceType::T32xlarge);
    }

    #[test]
    #[should_panic(expected = "Invalid instance type: invalid")]
    fn test_instance_type_from_str_invalid() {
        let _ = InstanceType::from("invalid");
    }

    #[test]
    fn test_from_resources_fits_t3_nano_small_request() {
        assert_eq!(
            InstanceType::from_resources(500, 512),
            Some(InstanceType::T3Nano)
        );
    }

    #[test]
    fn test_from_resources_fits_t3_nano_exact_request() {
        assert_eq!(
            InstanceType::from_resources(2000, 512),
            Some(InstanceType::T3Nano)
        );
    }

    #[test]
    fn test_from_resources_fits_t3_micro_mem_overflow() {
        assert_eq!(
            InstanceType::from_resources(2000, 513),
            Some(InstanceType::T3Micro)
        );
    }

    #[test]
    fn test_from_resources_fits_t3_medium_cpu_overflow() {
        assert_eq!(
            InstanceType::from_resources(2001, 8192),
            Some(InstanceType::T3Xlarge)
        );
    }

    #[test]
    fn test_from_resources_fits_t3_xlarge_exact() {
        assert_eq!(
            InstanceType::from_resources(4000, 16384),
            Some(InstanceType::T3Xlarge)
        );
    }

    #[test]
    fn test_from_resources_fits_t3_2xlarge_mem_overflow() {
        assert_eq!(
            InstanceType::from_resources(4000, 16385),
            Some(InstanceType::T32xlarge)
        );
    }

    #[test]
    fn test_from_resources_fits_t3_2xlarge_exact_request() {
        assert_eq!(
            InstanceType::from_resources(8000, 32768),
            Some(InstanceType::T32xlarge)
        );
    }

    #[test]
    fn test_from_resources_no_fit_cpu_overflow() {
        assert_eq!(InstanceType::from_resources(8001, 32768), None);
    }

    #[test]
    fn test_from_resources_no_fit_mem_overflow() {
        assert_eq!(InstanceType::from_resources(8000, 32769), None);
    }

    #[test]
    fn test_from_resources_no_fit_large_request() {
        assert_eq!(InstanceType::from_resources(u32::MAX, u64::MAX), None);
    }
}
