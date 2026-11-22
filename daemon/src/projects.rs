use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use uuid::Uuid;

use crate::accounts;
use crate::context::{self, ContextFiles};
use crate::private_fs;
use crate::protocol::{ErrorCode, Project, ProjectSettings};

#[derive(Debug, thiserror::Error)]
pub enum ProjectsError {
    #[error("project not found")]
    NotFound,
    #[error("project name is empty")]
    EmptyName,
    #[error("folder does not exist: {0}")]
    InvalidRoot(String),
    #[error("unknown Claude account: {0}")]
    UnknownAccount(String),
    #[error("unknown SSH key: {0}")]
    UnknownSshKey(String),
    #[error("context folder does not exist: {0}")]
    InvalidContextDir(String),
    #[error("failed to save projects: {0}")]
    Io(#[from] io::Error),
    #[error("project list lock poisoned")]
    Poisoned,
}

impl ProjectsError {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::NotFound => ErrorCode::ProjectNotFound,
            Self::InvalidRoot(_) | Self::InvalidContextDir(_) => ErrorCode::InvalidCwd,
            Self::EmptyName | Self::UnknownAccount(_) | Self::UnknownSshKey(_) => {
                ErrorCode::BadRequest
            }
            Self::Io(_) | Self::Poisoned => ErrorCode::Internal,
        }
    }
}

/// Projects saved in `projects.json`. Accounts and keys are checked against
/// what exists in the user's home, so a project can only point at real ones.
/// Context files of projects without their own folder live in
/// `contexts/<id>` next to `projects.json`.
pub struct Projects {
    file: PathBuf,
    home: PathBuf,
    entries: Mutex<Vec<Project>>,
}

impl Projects {
    pub fn load(file: PathBuf, home: PathBuf) -> Self {
        let entries = match fs::read(&file) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                eprintln!("ignoring unreadable {}: {error}", file.display());
                Vec::new()
            }),
            Err(_) => Vec::new(),
        };
        Self {
            file,
            home,
            entries: Mutex::new(entries),
        }
    }

    pub fn list(&self) -> Vec<Project> {
        self.entries
            .lock()
            .map(|entries| entries.clone())
            .unwrap_or_default()
    }

    pub fn get(&self, id: &str) -> Option<Project> {
        self.list().into_iter().find(|project| project.id == id)
    }

    pub fn create(&self, settings: ProjectSettings) -> Result<Project, ProjectsError> {
        let settings = self.validate(settings)?;
        let project = Project {
            id: Uuid::new_v4().to_string(),
            name: settings.name,
            root: settings.root,
            claude_config_dir: settings.claude_config_dir,
            ssh_key: settings.ssh_key,
            context_dir: settings.context_dir,
        };
        let mut entries = self.lock()?;
        entries.push(project.clone());
        self.save(&entries)?;
        Ok(project)
    }

    pub fn update(&self, id: &str, settings: ProjectSettings) -> Result<Project, ProjectsError> {
        let settings = self.validate(settings)?;
        let mut entries = self.lock()?;
        let project = entries
            .iter_mut()
            .find(|project| project.id == id)
            .ok_or(ProjectsError::NotFound)?;
        project.name = settings.name;
        project.root = settings.root;
        project.claude_config_dir = settings.claude_config_dir;
        project.ssh_key = settings.ssh_key;
        project.context_dir = settings.context_dir;
        let updated = project.clone();
        self.save(&entries)?;
        Ok(updated)
    }

    /// Sessions of a deleted project keep running and keep its environment.
    /// Its default context folder goes with it; a folder the user chose stays.
    pub fn delete(&self, id: &str) -> Result<(), ProjectsError> {
        let mut entries = self.lock()?;
        let before = entries.len();
        entries.retain(|project| project.id != id);
        if entries.len() == before {
            return Err(ProjectsError::NotFound);
        }
        self.save(&entries)?;
        match fs::remove_dir_all(self.default_context_dir(id)) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error.into()),
            _ => Ok(()),
        }
    }

    /// Where the project's rules and plan live.
    pub fn context_dir(&self, project: &Project) -> PathBuf {
        project
            .context_dir
            .as_ref()
            .map_or_else(|| self.default_context_dir(&project.id), PathBuf::from)
    }

    pub fn read_context(&self, id: &str) -> Result<(PathBuf, ContextFiles), ProjectsError> {
        let project = self.get(id).ok_or(ProjectsError::NotFound)?;
        let dir = self.context_dir(&project);
        let files = context::read(&dir)?;
        Ok((dir, files))
    }

    pub fn save_context(&self, id: &str, files: &ContextFiles) -> Result<PathBuf, ProjectsError> {
        let project = self.get(id).ok_or(ProjectsError::NotFound)?;
        let dir = self.context_dir(&project);
        context::write(&dir, files)?;
        Ok(dir)
    }

    fn default_context_dir(&self, id: &str) -> PathBuf {
        let data = self.file.parent().unwrap_or(Path::new("."));
        data.join("contexts").join(id)
    }

    fn validate(&self, settings: ProjectSettings) -> Result<ProjectSettings, ProjectsError> {
        let name = settings.name.trim().to_owned();
        if name.is_empty() {
            return Err(ProjectsError::EmptyName);
        }
        if !Path::new(&settings.root).is_dir() {
            return Err(ProjectsError::InvalidRoot(settings.root));
        }
        if let Some(dir) = &settings.claude_config_dir
            && !accounts::is_known_config_dir(&self.home, dir)
        {
            return Err(ProjectsError::UnknownAccount(dir.clone()));
        }
        if let Some(key) = &settings.ssh_key
            && !accounts::is_known_ssh_key(&self.home, key)
        {
            return Err(ProjectsError::UnknownSshKey(key.clone()));
        }
        if let Some(dir) = &settings.context_dir
            && !Path::new(dir).is_dir()
        {
            return Err(ProjectsError::InvalidContextDir(dir.clone()));
        }
        Ok(ProjectSettings { name, ..settings })
    }

    fn save(&self, entries: &[Project]) -> Result<(), ProjectsError> {
        if let Some(dir) = self.file.parent() {
            private_fs::create_dir(dir)?;
        }
        let json = serde_json::to_vec_pretty(entries).map_err(io::Error::other)?;
        private_fs::write_atomic(&self.file, &json)?;
        Ok(())
    }

    fn lock(&self) -> Result<MutexGuard<'_, Vec<Project>>, ProjectsError> {
        self.entries.lock().map_err(|_| ProjectsError::Poisoned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cohors-projects-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("dir");
        dir
    }

    fn settings(name: &str, root: &Path) -> ProjectSettings {
        ProjectSettings {
            name: name.into(),
            root: root.display().to_string(),
            claude_config_dir: None,
            ssh_key: None,
            context_dir: None,
        }
    }

    #[test]
    fn create_update_delete_and_reload() {
        let dir = temp_dir();
        let file = dir.join("projects.json");
        let projects = Projects::load(file.clone(), dir.clone());
        let created = projects.create(settings("  api ", &dir)).expect("create");
        assert_eq!(created.name, "api");

        projects
            .update(&created.id, settings("api v2", &dir))
            .expect("update");
        let reloaded = Projects::load(file.clone(), dir.clone());
        assert_eq!(reloaded.list()[0].name, "api v2");

        reloaded.delete(&created.id).expect("delete");
        assert!(Projects::load(file, dir.clone()).list().is_empty());
        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn rejects_invalid_settings() {
        let dir = temp_dir();
        let projects = Projects::load(dir.join("projects.json"), dir.clone());
        assert!(matches!(
            projects.create(settings(" ", &dir)),
            Err(ProjectsError::EmptyName)
        ));
        assert!(matches!(
            projects.create(settings("x", Path::new("/not/here"))),
            Err(ProjectsError::InvalidRoot(_))
        ));
        let unknown_account = ProjectSettings {
            claude_config_dir: Some("/nowhere".into()),
            ..settings("x", &dir)
        };
        assert!(matches!(
            projects.create(unknown_account),
            Err(ProjectsError::UnknownAccount(_))
        ));
        let unknown_key = ProjectSettings {
            ssh_key: Some("/nowhere/id".into()),
            ..settings("x", &dir)
        };
        assert!(matches!(
            projects.create(unknown_key),
            Err(ProjectsError::UnknownSshKey(_))
        ));
        let missing_context = ProjectSettings {
            context_dir: Some("/nowhere".into()),
            ..settings("x", &dir)
        };
        assert!(matches!(
            projects.create(missing_context),
            Err(ProjectsError::InvalidContextDir(_))
        ));
        assert!(matches!(
            projects.delete("missing"),
            Err(ProjectsError::NotFound)
        ));
        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn context_lives_in_data_until_the_project_is_deleted() {
        let dir = temp_dir();
        let projects = Projects::load(dir.join("projects.json"), dir.clone());
        let project = projects.create(settings("api", &dir)).expect("create");
        let (context_dir, empty) = projects.read_context(&project.id).expect("read");
        assert_eq!(context_dir, dir.join("contexts").join(&project.id));
        assert_eq!(empty, ContextFiles::default());

        let files = ContextFiles {
            rules: "Use tabs.".into(),
            plan: "1. Login".into(),
        };
        projects.save_context(&project.id, &files).expect("save");
        assert_eq!(projects.read_context(&project.id).expect("read").1, files);

        projects.delete(&project.id).expect("delete");
        assert!(!context_dir.exists());
        assert!(matches!(
            projects.read_context(&project.id),
            Err(ProjectsError::NotFound)
        ));
        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn chosen_context_folder_is_kept_on_delete() {
        let dir = temp_dir();
        let own = dir.join("notes");
        fs::create_dir_all(&own).expect("own dir");
        let projects = Projects::load(dir.join("projects.json"), dir.clone());
        let project = projects
            .create(ProjectSettings {
                context_dir: Some(own.display().to_string()),
                ..settings("api", &dir)
            })
            .expect("create");
        let saved = projects
            .save_context(&project.id, &ContextFiles::default())
            .expect("save");
        assert_eq!(saved, own);
        projects.delete(&project.id).expect("delete");
        assert!(own.join(context::RULES_FILE).exists());
        fs::remove_dir_all(dir).expect("cleanup");
    }
}
