use serde::{Deserialize, Serialize};
use shell_escape::unix::escape;

use crate::client::ExedevClient;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VmNewArgs {
    pub name: Option<String>,
    pub image: Option<String>,
    pub disk: Option<String>,
    pub command: Option<String>,
    pub integrations: Vec<String>,
    pub env: Vec<(String, String)>,
    pub no_email: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VmResizeSpec {
    pub disk: Option<String>,
}

fn quote(s: &str) -> String {
    escape(std::borrow::Cow::Borrowed(s)).into_owned()
}

impl ExedevClient {
    pub async fn vm_ls(&self) -> Result<serde_json::Value, crate::Error> {
        self.exec_json("ls -la").await
    }

    pub async fn vm_new(&self, args: VmNewArgs) -> Result<serde_json::Value, crate::Error> {
        let mut cmd = String::from("new");
        if let Some(name) = &args.name {
            cmd.push_str(&format!(" --name {}", quote(name)));
        }
        if let Some(image) = &args.image {
            cmd.push_str(&format!(" --image {}", quote(image)));
        }
        if let Some(disk) = &args.disk {
            cmd.push_str(&format!(" --disk {}", quote(disk)));
        }
        if let Some(command) = &args.command {
            cmd.push_str(&format!(" --command {}", quote(command)));
        }
        for integration in &args.integrations {
            cmd.push_str(&format!(" --integration {}", quote(integration)));
        }
        for (k, v) in &args.env {
            cmd.push_str(&format!(" --env {}", quote(&format!("{k}={v}"))));
        }
        if args.no_email {
            cmd.push_str(" --no-email");
        }
        self.exec_json(&cmd).await
    }

    pub async fn vm_rm(&self, name: &str) -> Result<(), crate::Error> {
        let cmd = format!("rm {}", quote(name));
        self.exec_raw(&cmd).await.map(|_| ())
    }

    pub async fn vm_stat(&self, name: &str) -> Result<serde_json::Value, crate::Error> {
        let cmd = format!("stat {}", quote(name));
        self.exec_json(&cmd).await
    }

    pub async fn vm_rename(&self, old: &str, new: &str) -> Result<(), crate::Error> {
        let cmd = format!("rename {} {}", quote(old), quote(new));
        self.exec_raw(&cmd).await.map(|_| ())
    }

    pub async fn vm_tag(&self, name: &str, tag: &str) -> Result<(), crate::Error> {
        let cmd = format!("tag {} {}", quote(name), quote(tag));
        self.exec_raw(&cmd).await.map(|_| ())
    }

    pub async fn vm_resize(
        &self,
        name: &str,
        spec: VmResizeSpec,
    ) -> Result<serde_json::Value, crate::Error> {
        let mut cmd = format!("resize {}", quote(name));
        if let Some(disk) = &spec.disk {
            cmd.push_str(&format!(" --disk {}", quote(disk)));
        }
        self.exec_json(&cmd).await
    }

    pub async fn whoami(&self) -> Result<serde_json::Value, crate::Error> {
        self.exec_json("whoami").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_handles_spaces_and_quotes() {
        assert_eq!(quote("simple"), "simple");
        let q = quote("with space");
        assert!(q.contains(' '));
        assert!(q.starts_with('\'') || q.starts_with('"'));
        let q = quote("it's");
        assert!(q.contains("it"));
    }
}
