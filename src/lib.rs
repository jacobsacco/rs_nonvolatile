
use whoami;
use whoami::Platform::{Linux, Windows};
use serde::{Serialize, Deserialize};
use serde_yaml;
use std::fs::{
	copy,
	create_dir_all, 
	rename, 
	metadata,
	read_to_string, 
	OpenOptions,
	remove_file,
	remove_dir_all,
	canonicalize,
	is_file,
};
use std::path::Path;
use std::collections::HashMap;
use std::env;
use std::io::Write;
use std::process;
use sysinfo;
use generic_error::{Result, GenErr, GenericError};
use fs_util::copy_dir;


#[cfg(test)]
mod tests;


#[derive(Serialize, Deserialize, Debug)]
pub struct State {
	name: String,
	path: String,
	manifest_path: String,
	tmp_manifest_path: String,
	items: HashMap<String, String>,
	preserved: HashMap<String, String>,
}


fn build_var_path(var: &str, sub_dir: &str) -> Result<String> {
	let s = env::var(var)?;
	Ok(format!("{}/{}", s, sub_dir))
}


fn get_storage_dir() -> Result<String> {
	match whoami::platform() {
		Linux => {
			build_var_path("HOME", ".local/rust_nonvolatile")
		},
		Windows => {
			build_var_path("appdata", "rust_nonvolatile")
		},
		_ => GenErr!("nonvolatile: {} not supported", whoami::platform()),
	}
}


fn acquire_dir(storage_dir: String) -> Result<()> {
	let lockfile_path = Path::new(format!("{}/{}", storage_dir, "~rust_nonvolatile.lock"));
	if is_file(lockfile_path) {
		let pid = process::id();
		read the file
		if file contents == pid {
			return Ok(());
		} 
		let mut system = sysinfo::System::new();
		system.refresh_processes();
		for (other_pid, _proc) in system.get_process_list() {
			if other_pid == pid {
				return GenErr!("Can't acquire storage directory {}; already owned by process {}", storage_dir, other_pid);
			}
		}
		let _ = remove_file(lockfile_path);
	}
	write pid to lockfile_path
	Ok(())
}


fn release_dir(String) {
	let lockfile_path = Path::new(format!("{}/{}", storage_dir, "~rust_nonvolatile.lock"));
	let _ = remove_file(lockfile_path);
}


impl State {

	fn write_manifest(&self) -> Result<()> {
		let mut file = OpenOptions::new().write(true).create(true).open(&self.tmp_manifest_path)?;
		let data = serde_yaml::to_vec(self)?;
		file.write(&data)?;
		rename(&self.tmp_manifest_path, &self.manifest_path)?;
		Ok(())
	}
	
	
	pub fn set(&mut self, var: &str, value: &str) -> Result<()> {
		if self.preserved.contains_key(var) {
			return GenErr!("nonvolatile: can't set a variable with the same name as a preserved file/folder");
		}
		let _ = self.items.insert(String::from(var), String::from(value));
		self.write_manifest()
	}
	
	
	pub fn get(&self, var: &str) -> Option<String> {
		let item = self.items.get(var)?;
		Some(String::from(item))
	}
	
	
	pub fn delete(&mut self, name: &str) -> Result<()> {
		let _ = self.items.remove(name);
		if let Some(_) = self.preserved.remove(name) {
			let path = format!("{}/{}", &self.path, name);
			remove_file(&path)?;
			remove_dir_all(&path)?;
		}
		self.write_manifest()
	}
	
	
	pub fn new(name: &str) -> Result<State> {
		let dir = get_storage_dir()?;
		let path = format!("{}/{}", &dir, name);
		create_dir_all(&path)?;
		let items: HashMap<String, String> = HashMap::new();
		let preserved: HashMap<String, String> = HashMap::new();
		let state = State {
			name: String::from(name),
			path: path.clone(),
			manifest_path: format!("{}/{}", &path, ".manifest"),
			tmp_manifest_path: format!("{}/{}", &path, ".manifest_tmp"),
			items: items,
			preserved: preserved,
		};
		State::write_manifest(&state)?;
		Ok(state)
	}
	
	
	pub fn load(name: &str) -> Result<State> {
		let dir = get_storage_dir()?;
		let path = format!("{}/{}", &dir, name);
		let manifest_path = format!("{}/{}", &path, ".manifest");
		let data = read_to_string(&manifest_path)?;
		let state: State = serde_yaml::from_str(&data)?;
		Ok(state)
	}
	
	
	pub fn load_else_create(name: &str) -> Result<State> {
		State::load(name).or_else(|_| State::new(name))
	}
	
	
	pub fn destroy_state(name: &str) {
		if let Ok(dir) = get_storage_dir() {
			let path = format!("{}/{}", dir, name);
			let _ = remove_dir_all(path);
		} 
	}
	
	
	pub fn has(&self, item: &str) -> bool {
		self.items.contains_key(item) || self.preserved.contains_key(item)
	}
	
	
	pub fn preserve(&mut self, path: &str, name: &str) -> Result<()> {
		if self.items.contains_key(name) {
			return GenErr!("nonvolatile: can't preserve a file with the same name as a set variable");
		}
		let path = match canonicalize(path)?.to_str() {
			Some(p) => String::from(p),
			None => return GenErr!("nonvolatile preserve: failed to canonicalize path"),
		};
		let tmp_name = format!("tmp_{}", name);
		let tmp_dest = format!("{}/{}", &self.path, &tmp_name);
		let dest = format!("{}/{}", &self.path, name);
		if metadata(&path)?.is_dir() {
			copy_dir(&path, &tmp_dest)?;
		}
		else {
			copy(&path, &tmp_dest)?;
		}
		
		let _ = self.preserved.insert(String::from(name), String::from(path));
		self.write_manifest()?;
		
		rename(tmp_dest, dest)?;
		Ok(())
	}


	pub fn restore(&self, name: &str) -> Result<()> {
		let path = match self.preserved.get(name) {
			Some(p) => p,
			None => return GenErr!("Nothing by the name '{}' has been preserved", name),
		};
		self.restore_to(name, path)
	}
	
	
	pub fn restore_to(&self, name: &str, path: &str) -> Result<()> {
		if !self.preserved.contains_key(name) {
			return GenErr!("Nothing by the name '{}' has been preserved", name);
		}
		let preserved_path = format!("{}/{}", &self.path, name);
		if metadata(&preserved_path)?.is_dir() {
			copy_dir(&preserved_path, path)?;
		}
		else {
			copy(&preserved_path, path)?;
		}
		Ok(())
	}
}



