/// I borrowed the error type of `rust-libp2p`, deleted some error types, and added an error type.
use std::{error, fmt, io};

/// Error at the SECIO layer communication.
#[derive(Debug)]
pub enum SecioError {
    /// I/O error.
    IoError(io::Error),

    /// Failed to generate ephemeral key.
    EphemeralKeyGenerationFailed,

    /// Failed to generate the secret shared key from the ephemeral key.
    SecretGenerationFailed,

    /// There is no protocol supported by both the local and remote hosts.
    NoSupportIntersection,

    /// The final check of the handshake failed.
    NonceVerificationFailed,

    /// The received frame was of invalid length.
    FrameTooShort,

    /// The hashes of the message didn't match.
    HmacNotMatching,

    /// Connect yourself
    ConnectSelf,

    /// Failed to parse one of the handshake bincode messages.
    HandshakeParsingFailure,

    /// The signature of the exchange packet doesn't verify the remote public key.
    SignatureVerificationFailed,

    /// Invalid message message found during handshake
    InvalidMessage,

    /// We received an invalid proposition from remote.
    InvalidProposition(&'static str),
}

impl From<io::Error> for SecioError {
    #[inline]
    fn from(err: io::Error) -> SecioError {
        SecioError::IoError(err)
    }
}

impl Into<io::Error> for SecioError {
    fn into(self) -> io::Error {
        match self {
            SecioError::IoError(e) => e,
            e => io::Error::new(io::ErrorKind::BrokenPipe, error::Error::description(&e)),
        }
    }
}

impl error::Error for SecioError {
    fn description(&self) -> &str {
        match self {
            SecioError::IoError(e) => error::Error::description(e),
            SecioError::EphemeralKeyGenerationFailed => "EphemeralKey Generation Failed",
            SecioError::SecretGenerationFailed => "Secret Generation Failed",
            SecioError::NoSupportIntersection => "No Support Intersection",
            SecioError::NonceVerificationFailed => "Nonce Verification Failed",
            SecioError::FrameTooShort => "Frame Too Short",
            SecioError::HmacNotMatching => "Hmac Not Matching",
            SecioError::ConnectSelf => "Connect Self",
            SecioError::HandshakeParsingFailure => "Handshake Parsing Failure",
            SecioError::InvalidMessage => "Invalid Message",
            SecioError::SignatureVerificationFailed => "Signature Verification Failed",
            SecioError::InvalidProposition(e) => e,
        }
    }
}

impl fmt::Display for SecioError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SecioError::IoError(e) => fmt::Display::fmt(&e, f),
            SecioError::EphemeralKeyGenerationFailed => write!(f, "EphemeralKey Generation Failed"),
            SecioError::SecretGenerationFailed => write!(f, "Secret Generation Failed"),
            SecioError::NoSupportIntersection => write!(f, "No Support Intersection"),
            SecioError::NonceVerificationFailed => write!(f, "Nonce Verification Failed"),
            SecioError::FrameTooShort => write!(f, "Frame Too Short"),
            SecioError::HmacNotMatching => write!(f, "Hmac Not Matching"),
            SecioError::ConnectSelf => write!(f, "Connect Self"),
            SecioError::HandshakeParsingFailure => write!(f, "Handshake Parsing Failure"),
            SecioError::InvalidMessage => write!(f, "Invalid Message"),
            SecioError::SignatureVerificationFailed => write!(f, "Signature Verification Failed"),
            SecioError::InvalidProposition(e) => write!(f, "Invalid Proposition: {}", e),
        }
    }
}
