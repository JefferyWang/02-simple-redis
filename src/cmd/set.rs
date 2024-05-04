use crate::{RespArray, RespFrame};

use super::{extract_args, validate_command, CommandError, CommandExecutor, SAdd, SisMember};

impl CommandExecutor for SAdd {
    fn execute(self, backend: &crate::Backend) -> RespFrame {
        backend.sadd(self.key, self.member);
        RespFrame::Integer(1)
    }
}

impl CommandExecutor for SisMember {
    fn execute(self, backend: &crate::Backend) -> RespFrame {
        let result = backend.sismember(&self.key, &self.member);
        RespFrame::Integer(if result { 1 } else { 0 })
    }
}

impl TryFrom<RespArray> for SAdd {
    type Error = CommandError;

    fn try_from(value: RespArray) -> Result<Self, Self::Error> {
        validate_command(&value, &["sadd"], 2)?;

        let mut args = extract_args(value, 1)?.into_iter();
        match (args.next(), args.next()) {
            (Some(RespFrame::BulkString(key)), Some(RespFrame::BulkString(member))) => Ok(SAdd {
                key: String::from_utf8(key.0)?,
                member: String::from_utf8(member.0)?,
            }),
            _ => Err(CommandError::InvalidArgument(
                "Invalid key or value".to_string(),
            )),
        }
    }
}

impl TryFrom<RespArray> for SisMember {
    type Error = CommandError;

    fn try_from(value: RespArray) -> Result<Self, Self::Error> {
        validate_command(&value, &["sismember"], 2)?;

        let mut args = extract_args(value, 1)?.into_iter();
        match (args.next(), args.next()) {
            (Some(RespFrame::BulkString(key)), Some(RespFrame::BulkString(member))) => {
                Ok(SisMember {
                    key: String::from_utf8(key.0)?,
                    member: String::from_utf8(member.0)?,
                })
            }
            _ => Err(CommandError::InvalidArgument(
                "Invalid key or value".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::RespDecode;

    use super::*;
    use anyhow::Result;
    use bytes::BytesMut;

    #[test]
    fn test_sadd_from_resp_array() -> Result<()> {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(b"*3\r\n$4\r\nsadd\r\n$3\r\nkey\r\n$6\r\nmember\r\n");

        let frame = RespArray::decode(&mut buf)?;

        let result: SAdd = frame.try_into()?;
        assert_eq!(result.key, "key");
        assert_eq!(result.member, "member");

        Ok(())
    }

    #[test]
    fn test_sismember_from_resp_array() -> Result<()> {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(b"*3\r\n$9\r\nsismember\r\n$3\r\nkey\r\n$6\r\nmember\r\n");

        let frame = RespArray::decode(&mut buf)?;

        let result: SisMember = frame.try_into()?;
        assert_eq!(result.key, "key");
        assert_eq!(result.member, "member");

        Ok(())
    }

    #[test]
    fn test_sadd_sismember_commands() -> Result<()> {
        let backend = crate::Backend::new();
        let cmd = SAdd {
            key: "key".to_string(),
            member: "member".to_string(),
        };
        let result = cmd.execute(&backend);
        assert_eq!(result, RespFrame::Integer(1));

        let cmd = SisMember {
            key: "key".to_string(),
            member: "member".to_string(),
        };
        let result = cmd.execute(&backend);
        assert_eq!(result, RespFrame::Integer(1));

        let cmd = SisMember {
            key: "key".to_string(),
            member: "member1".to_string(),
        };
        let result = cmd.execute(&backend);
        assert_eq!(result, RespFrame::Integer(0));

        Ok(())
    }
}
