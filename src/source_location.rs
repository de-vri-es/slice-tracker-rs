use std::path::PathBuf;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum SourceLocation<'a, B> where B: 'a + ?Sized {
	Unknown,
	ExpandedFrom(&'a B),
	File(PathBuf),
}
