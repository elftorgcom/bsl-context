//! Singleton-защита через PID-lock.
//!
//! Гарантирует, что у `bsl-context-rs` ровно один процесс на машине. Без этого
//! второй экземпляр успевает 5 секунд крутить cold-start (парсинг hbk),
//! расходовать RAM/CPU, и только потом упасть на `bind 10048`. С этим locks
//! второй экземпляр выходит до загрузки индекса с понятным сообщением о PID
//! уже работающего инстанса.
//!
//! Порт из `code-index/crates/code-index-core/src/daemon_core/lock.rs` с одним
//! отличием: дополнительно сверяем имя процесса (карточка #2424 — на Windows
//! ОС переиспользует PID после reboot, проверка только PID даёт ложное «уже
//! запущен»).
//!
//! Файл-лок — `<log_dir>/bsl-context-rs.pid` (`log_dir` уже создаётся `run.bat`,
//! значит каталог гарантированно существует).

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

const PID_FILE_NAME: &str = "bsl-context-rs.pid";
const EXPECTED_PROC_NAME: &str = "bsl-context-rs.exe";

/// RAII-guard PID-lock. Удаляет файл в `Drop`.
pub struct PidLock {
    path: PathBuf,
}

impl PidLock {
    /// Захватить PID-lock в каталоге `log_dir`. Если файл существует и процесс
    /// с записанным PID жив И его имя совпадает с `bsl-context-rs.exe` —
    /// возвращается ошибка с указанием PID. Stale-файл (мёртвый PID или чужое
    /// имя процесса) перезаписывается.
    pub fn acquire(log_dir: &Path) -> Result<Self> {
        let pid_path = log_dir.join(PID_FILE_NAME);

        if pid_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&pid_path) {
                if let Ok(pid) = content.trim().parse::<u32>() {
                    if is_our_process_alive(pid) {
                        bail!(
                            "Сервис bsl-context-rs уже запущен (PID {}). PID-файл: {}. \
                             Если это ошибочное срабатывание — удалите файл или дождитесь его \
                             автоудаления при graceful shutdown.",
                            pid,
                            pid_path.display()
                        );
                    }
                }
            }
            tracing::warn!(
                "найден устаревший PID-файл {} — перезаписываем",
                pid_path.display()
            );
        }

        std::fs::write(&pid_path, std::process::id().to_string())?;
        Ok(Self { path: pid_path })
    }
}

impl Drop for PidLock {
    fn drop(&mut self) {
        // Удаление лучшее усилие — если упал не мы, а ОС, ничего не поделать.
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Проверить, что процесс `pid` жив И его имя совпадает с `bsl-context-rs.exe`.
///
/// Двойная проверка нужна потому, что Windows переиспользует PID после
/// перезагрузки. Если PID совпал, но имя exe другое — это чужой процесс,
/// stale-файл, можем перезаписать.
fn is_our_process_alive(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessesToUpdate, System};

    let mut sys = System::new();
    let spid = Pid::from(pid as usize);
    sys.refresh_processes(ProcessesToUpdate::Some(&[spid]), false);

    let Some(proc) = sys.process(spid) else {
        return false;
    };
    let name = proc.name().to_string_lossy().to_lowercase();
    name == EXPECTED_PROC_NAME.to_lowercase()
        // На случай, если sysinfo вернёт имя без расширения — допускаем «голое» имя.
        || name == EXPECTED_PROC_NAME.trim_end_matches(".exe").to_lowercase()
}
