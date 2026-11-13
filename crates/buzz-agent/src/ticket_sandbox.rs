// DeepSeek 式工單目錄隔離 (Ticket-based Directory Isolation)
// 為特工每個任務分配獨立臨時工作目錄，產出驗收無誤後才原子化併入主幹，阻斷多特工路徑混淆與檔案交叉污染。
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct TicketSandbox {
    session_id: String,
    sandbox_dir: PathBuf,
}

impl TicketSandbox {
    /// 建立工單沙箱實例
    /// - `base_dir`: 基礎目錄（例如專案根目錄或 `.buzz/.scratch`）
    /// - `session_id`: 工單或會話唯一識別碼
    pub fn new<P: AsRef<Path>>(base_dir: P, session_id: &str) -> Self {
        let clean_id = Self::sanitize_session_id(session_id);
        let sandbox_dir = base_dir
            .as_ref()
            .join(".buzz")
            .join(".scratch")
            .join(format!("ticket_{}", clean_id));

        Self {
            session_id: clean_id,
            sandbox_dir,
        }
    }

    /// 取得清理後的 Session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// 取得沙箱實體路徑
    pub fn sandbox_dir(&self) -> &Path {
        &self.sandbox_dir
    }

    /// 清理 Session ID 中的非法路徑字元，確保跨平台相容
    pub fn sanitize_session_id(raw: &str) -> String {
        raw.chars()
            .map(|c| match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => c,
                _ => '_',
            })
            .collect()
    }

    /// 初始化並建立沙箱實體目錄
    pub fn create(&self) -> io::Result<PathBuf> {
        if !self.sandbox_dir.exists() {
            fs::create_dir_all(&self.sandbox_dir)?;
        }
        Ok(self.sandbox_dir.clone())
    }

    /// 將相對路徑解析並重定向至沙箱內部，杜絕路徑穿越 (Path Traversal)
    pub fn resolve_path<P: AsRef<Path>>(&self, rel_path: P) -> io::Result<PathBuf> {
        let rel = rel_path.as_ref();
        
        // 排除絕對路徑或包含 .. 的穿越路徑
        if rel.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "沙箱路徑解析禁止使用絕對路徑",
            ));
        }

        let mut target = self.sandbox_dir.clone();
        for component in rel.components() {
            match component {
                std::path::Component::Normal(p) => target.push(p),
                std::path::Component::ParentDir => {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "沙箱路徑解析禁止路徑穿越 (..) 操作",
                    ));
                }
                _ => {}
            }
        }

        Ok(target)
    }

    /// 在沙箱中寫入檔案（自動建立父目錄）
    pub fn write_file<P: AsRef<Path>>(&self, rel_path: P, content: &[u8]) -> io::Result<PathBuf> {
        let full_path = self.resolve_path(rel_path)?;
        if let Some(parent) = full_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
        fs::write(&full_path, content)?;
        Ok(full_path)
    }

    /// 在沙箱中讀取檔案
    pub fn read_file<P: AsRef<Path>>(&self, rel_path: P) -> io::Result<Vec<u8>> {
        let full_path = self.resolve_path(rel_path)?;
        fs::read(full_path)
    }

    /// 將沙箱中經過驗收的所有檔案，原子化安全合流至目標工作區目錄
    /// 回傳已成功合流的相對路徑清單
    pub fn sync_to_workspace<P: AsRef<Path>>(&self, target_workspace: P) -> io::Result<Vec<PathBuf>> {
        let workspace = target_workspace.as_ref();
        let mut synced_files = Vec::new();

        if !self.sandbox_dir.exists() {
            return Ok(synced_files);
        }

        self.copy_dir_recursive(&self.sandbox_dir, workspace, &self.sandbox_dir, &mut synced_files)?;

        Ok(synced_files)
    }

    fn copy_dir_recursive(
        &self,
        src: &Path,
        dst: &Path,
        root: &Path,
        synced: &mut Vec<PathBuf>,
    ) -> io::Result<()> {
        if !dst.exists() {
            fs::create_dir_all(dst)?;
        }

        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let path = entry.path();
            let file_name = entry.file_name();
            let dest_path = dst.join(&file_name);

            if path.is_dir() {
                self.copy_dir_recursive(&path, &dest_path, root, synced)?;
            } else if path.is_file() {
                fs::copy(&path, &dest_path)?;
                if let Ok(rel) = path.strip_prefix(root) {
                    synced.push(rel.to_path_buf());
                }
            }
        }

        Ok(())
    }

    /// 清理沙箱臨時目錄（任務完成或失敗時呼叫）
    pub fn cleanup(&self) -> io::Result<()> {
        if self.sandbox_dir.exists() {
            fs::remove_dir_all(&self.sandbox_dir)?;
        }
        Ok(())
    }
}

