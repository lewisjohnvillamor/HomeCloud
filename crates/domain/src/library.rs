//! Library membership rules.
//!
//! A library is the authorization boundary of the system: every file
//! belongs to exactly one library, and access is decided by membership.
//! These rules are enforced here and mirrored by database constraints so
//! neither layer alone can be bypassed.

use crate::identity::{LibraryId, UserId};
use crate::naming::LibraryName;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MembershipError {
    #[error("the library owner cannot be removed")]
    OwnerCannotBeRemoved,
    #[error("a library has exactly one owner")]
    OwnerAlreadyExists,
    #[error("the user is already a member of this library")]
    AlreadyMember,
    #[error("unknown role `{0}`")]
    UnknownRole(String),
}

/// What a member may do. Roles are coarse on purpose: finer-grained
/// capabilities belong to share grants, which are narrower than any
/// member role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryRole {
    /// Full control, including membership. Exactly one per library.
    Owner,
    /// Reads and writes library content, but not membership.
    Member,
}

impl LibraryRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            LibraryRole::Owner => "owner",
            LibraryRole::Member => "member",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, MembershipError> {
        match raw {
            "owner" => Ok(LibraryRole::Owner),
            "member" => Ok(LibraryRole::Member),
            other => Err(MembershipError::UnknownRole(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Membership {
    pub user: UserId,
    pub role: LibraryRole,
}

/// A library and its membership list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Library {
    id: LibraryId,
    name: LibraryName,
    members: Vec<Membership>,
}

impl Library {
    /// Creates a library. A library cannot exist without an owner, so the
    /// owner is supplied at construction rather than added afterwards.
    pub fn create(id: LibraryId, name: LibraryName, owner: UserId) -> Self {
        Self {
            id,
            name,
            members: vec![Membership {
                user: owner,
                role: LibraryRole::Owner,
            }],
        }
    }

    pub fn id(&self) -> LibraryId {
        self.id
    }

    pub fn name(&self) -> &LibraryName {
        &self.name
    }

    pub fn members(&self) -> &[Membership] {
        &self.members
    }

    /// The owner always exists; `create` establishes it and no operation
    /// can remove it.
    pub fn owner(&self) -> UserId {
        self.members
            .iter()
            .find(|member| member.role == LibraryRole::Owner)
            .map(|member| member.user)
            .expect("a library always has an owner")
    }

    pub fn role_of(&self, user: UserId) -> Option<LibraryRole> {
        self.members
            .iter()
            .find(|member| member.user == user)
            .map(|member| member.role)
    }

    pub fn add_member(&mut self, user: UserId, role: LibraryRole) -> Result<(), MembershipError> {
        if role == LibraryRole::Owner {
            return Err(MembershipError::OwnerAlreadyExists);
        }
        if self.role_of(user).is_some() {
            return Err(MembershipError::AlreadyMember);
        }

        self.members.push(Membership { user, role });
        Ok(())
    }

    /// Removing a non-member is a no-op rather than an error: the caller
    /// asked for a state that already holds.
    pub fn remove_member(&mut self, user: UserId) -> Result<(), MembershipError> {
        if self.role_of(user) == Some(LibraryRole::Owner) {
            return Err(MembershipError::OwnerCannotBeRemoved);
        }

        self.members.retain(|member| member.user != user);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> (Library, UserId) {
        let owner = UserId::new();
        let library = Library::create(
            LibraryId::new(),
            LibraryName::parse("Home").expect("valid name"),
            owner,
        );
        (library, owner)
    }

    #[test]
    fn a_new_library_is_owned_by_its_creator() {
        let (library, owner) = library();

        assert_eq!(library.owner(), owner);
        assert_eq!(library.role_of(owner), Some(LibraryRole::Owner));
        assert_eq!(library.members().len(), 1);
    }

    #[test]
    fn a_second_owner_cannot_be_added() {
        let (mut library, _) = library();

        assert_eq!(
            library.add_member(UserId::new(), LibraryRole::Owner),
            Err(MembershipError::OwnerAlreadyExists)
        );
    }

    #[test]
    fn members_are_not_added_twice() {
        let (mut library, _) = library();
        let guest = UserId::new();

        library
            .add_member(guest, LibraryRole::Member)
            .expect("first add succeeds");

        assert_eq!(
            library.add_member(guest, LibraryRole::Member),
            Err(MembershipError::AlreadyMember)
        );
    }

    #[test]
    fn the_owner_cannot_be_removed() {
        let (mut library, owner) = library();

        assert_eq!(
            library.remove_member(owner),
            Err(MembershipError::OwnerCannotBeRemoved)
        );
        assert_eq!(library.owner(), owner);
    }

    #[test]
    fn removing_a_member_leaves_the_rest_intact() {
        let (mut library, owner) = library();
        let guest = UserId::new();
        library
            .add_member(guest, LibraryRole::Member)
            .expect("add member");

        library.remove_member(guest).expect("remove member");

        assert_eq!(library.role_of(guest), None);
        assert_eq!(library.owner(), owner);
    }

    #[test]
    fn a_non_member_is_not_granted_a_role() {
        let (library, _) = library();

        assert_eq!(library.role_of(UserId::new()), None);
    }

    #[test]
    fn roles_round_trip_through_their_stored_form() {
        for role in [LibraryRole::Owner, LibraryRole::Member] {
            assert_eq!(LibraryRole::parse(role.as_str()), Ok(role));
        }

        assert!(LibraryRole::parse("administrator").is_err());
    }
}
