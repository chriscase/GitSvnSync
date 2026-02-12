//! Email notification sender via SMTP.
//!
//! Uses the `lettre` crate to send HTML-formatted notification emails.

use lettre::message::{header::ContentType, Mailbox};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use tracing::{debug, info, warn};

use crate::errors::NotificationError;

/// SMTP email notifier.
pub struct EmailNotifier {
    smtp_addr: String,
    from: String,
    recipients: Vec<String>,
}

impl EmailNotifier {
    /// Create a new email notifier.
    ///
    /// `smtp_addr` should be `host:port` (e.g. `smtp.example.com:587`).
    pub fn new(smtp_addr: String, from: String, recipients: Vec<String>) -> Self {
        info!(
            smtp = %smtp_addr,
            from = %from,
            recipients = ?recipients,
            "initializing email notifier"
        );
        Self {
            smtp_addr,
            from,
            recipients,
        }
    }

    /// Send an HTML email to all configured recipients.
    pub async fn send(&self, subject: &str, html_body: &str) -> Result<(), NotificationError> {
        debug!(subject, to = ?self.recipients, "sending email");

        let from_mailbox: Mailbox = self
            .from
            .parse()
            .map_err(|e| NotificationError::EmailError(format!("invalid from address: {}", e)))?;

        for recipient in &self.recipients {
            let to_mailbox: Mailbox = recipient.parse().map_err(|e| {
                NotificationError::EmailError(format!("invalid recipient '{}': {}", recipient, e))
            })?;

            let email = Message::builder()
                .from(from_mailbox.clone())
                .to(to_mailbox)
                .subject(subject)
                .header(ContentType::TEXT_HTML)
                .body(html_body.to_string())
                .map_err(|e| {
                    NotificationError::EmailError(format!("failed to build email: {}", e))
                })?;

            // Build the transport. We use STARTTLS by default.
            let transport = self.build_transport()?;

            match transport.send(email).await {
                Ok(_) => {
                    info!(to = %recipient, "email sent successfully");
                }
                Err(e) => {
                    warn!(to = %recipient, error = %e, "failed to send email");
                    return Err(NotificationError::EmailError(format!(
                        "SMTP send to '{}' failed: {}",
                        recipient, e
                    )));
                }
            }
        }

        Ok(())
    }

    /// Build an async SMTP transport from the configured address.
    fn build_transport(&self) -> Result<AsyncSmtpTransport<Tokio1Executor>, NotificationError> {
        // Parse host:port.
        let parts: Vec<&str> = self.smtp_addr.rsplitn(2, ':').collect();
        let (host, _port) = match parts.len() {
            2 => (parts[1], parts[0].parse::<u16>().unwrap_or(587)),
            _ => (self.smtp_addr.as_str(), 587),
        };

        let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
            .map_err(|e| NotificationError::EmailError(format!("SMTP connection error: {}", e)))?
            .build();

        Ok(transport)
    }

    /// Build a transport with credentials (username/password).
    #[allow(dead_code)]
    fn build_transport_with_credentials(
        &self,
        username: &str,
        password: &str,
    ) -> Result<AsyncSmtpTransport<Tokio1Executor>, NotificationError> {
        let parts: Vec<&str> = self.smtp_addr.rsplitn(2, ':').collect();
        let host = if parts.len() == 2 {
            parts[1]
        } else {
            &self.smtp_addr
        };

        let creds = Credentials::new(username.to_string(), password.to_string());

        let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
            .map_err(|e| NotificationError::EmailError(format!("SMTP connection error: {}", e)))?
            .credentials(creds)
            .build();

        Ok(transport)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_notifier_construction() {
        let notifier = EmailNotifier::new(
            "smtp.example.com:587".into(),
            "sync@example.com".into(),
            vec!["admin@example.com".into()],
        );
        assert_eq!(notifier.from, "sync@example.com");
        assert_eq!(notifier.recipients.len(), 1);
    }
}
