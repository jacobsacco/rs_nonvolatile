
use super::*;
use std::fs;


fn destroy_state() {
	println!("Destroying any pre-existing state");
	State::destroy_state("test");
	println!("Done");
}


fn test_state(s: &mut State) {
	println!("Testing if state object is functional");
	let _ = s.delete("test_var");
	assert_eq!(s.has("test_var"), false);
	s.set("test_var", "foo").unwrap();
	assert_eq!(s.has("test_var"), true);
	assert_eq!(s.get("test_var"), Some(String::from("foo")));
	println!("Done");
}


#[test]
#[should_panic]
fn test_destroy_state() {
	let s = match State::load_else_create("test") {
		Ok(s) => s,
		Err(e) => {
			println!("load_else_create failed! {:?}", e);
			return;
		}
	};
	
	drop(s);
	
	destroy_state();
	
	let _ = State::load("test").unwrap();
}


#[test]
fn test_new_state() {
	destroy_state();
	
	let mut s = State::new("test").unwrap();
	test_state(&mut s);
}


#[test]
fn test_load_state() {
	destroy_state();
	{
		let _ = State::new("test").unwrap();
	}
	let mut s = State::load("test").unwrap();
	test_state(&mut s);
}

#[test]
fn test_load_else_create() {
	destroy_state();
	
	let mut s = State::load_else_create("test").unwrap();
	test_state(&mut s);
}

#[test]
fn test_manifest_write() {
	destroy_state();
	{
		let mut s = State::load_else_create("test").unwrap();
		s.set("test manifest write", "foobar").unwrap();
	}
	
	{
		let mut s = State::load_else_create("test").unwrap();
		assert_eq!(s.get("test manifest write"), Some(String::from("foobar")));
		s.delete("test manifest write").unwrap();
	}
	
	{
		let s = State::load_else_create("test").unwrap();
		assert_eq!(s.has("test manifest write"), false);
	}
}

#[test]
fn test_has() {
	destroy_state();
	let mut s = State::load_else_create("test").unwrap();
	assert_eq!(s.has("something"), false);
	s.set("something", "something").unwrap();
	assert_eq!(s.has("something"), true);
	assert_eq!(s.has("something else"), false);
}

#[test]
fn test_delete_var() {
	destroy_state();
	let mut s = State::load_else_create("test").unwrap();
	s.set("foo", "bar").unwrap();
	assert_eq!(s.has("foo"), true);
	s.delete("foo").unwrap();
	assert_eq!(s.has("foo"), false);
}

#[test]
fn test_preserve_and_restore() {
	destroy_state();
	
	let mut _s = State::load_else_create("test").unwrap();
	println!("{:?}", fs::canonicalize("."));
	panic!("FUCK");
}