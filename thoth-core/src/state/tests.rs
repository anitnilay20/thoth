#[cfg(test)]
mod navigation_history_tests {
    use crate::state::NavigationHistory;

    #[test]
    fn test_new_history_is_empty() {
        let history = NavigationHistory::new();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        assert!(!history.can_go_back());
        assert!(!history.can_go_forward());
        assert!(history.current().is_none());
    }

    #[test]
    fn test_push_adds_to_history() {
        let mut history = NavigationHistory::new();

        history.push("0".to_string());
        assert_eq!(history.len(), 1);
        assert_eq!(history.current(), Some(&"0".to_string()));
        assert!(!history.can_go_back());
        assert!(!history.can_go_forward());

        history.push("0.user".to_string());
        assert_eq!(history.len(), 2);
        assert_eq!(history.current(), Some(&"0.user".to_string()));
        assert!(history.can_go_back());
        assert!(!history.can_go_forward());
    }

    #[test]
    fn test_push_same_path_does_not_duplicate() {
        let mut history = NavigationHistory::new();

        history.push("0".to_string());
        history.push("0".to_string());

        assert_eq!(history.len(), 1);
        assert_eq!(history.current(), Some(&"0".to_string()));
    }

    #[test]
    fn test_back_navigation() {
        let mut history = NavigationHistory::new();

        history.push("0".to_string());
        history.push("1".to_string());
        history.push("2".to_string());

        // Current is "2"
        assert_eq!(history.current(), Some(&"2".to_string()));

        // Go back to "1"
        assert_eq!(history.back(), Some("1".to_string()));
        assert_eq!(history.current(), Some(&"1".to_string()));
        assert!(history.can_go_back());
        assert!(history.can_go_forward());

        // Go back to "0"
        assert_eq!(history.back(), Some("0".to_string()));
        assert_eq!(history.current(), Some(&"0".to_string()));
        assert!(!history.can_go_back());
        assert!(history.can_go_forward());

        // Can't go back further
        assert_eq!(history.back(), None);
    }

    #[test]
    fn test_forward_navigation() {
        let mut history = NavigationHistory::new();

        history.push("0".to_string());
        history.push("1".to_string());
        history.push("2".to_string());

        // Go back twice
        history.back();
        history.back();

        // Current is "0"
        assert_eq!(history.current(), Some(&"0".to_string()));

        // Go forward to "1"
        assert_eq!(history.forward(), Some("1".to_string()));
        assert_eq!(history.current(), Some(&"1".to_string()));

        // Go forward to "2"
        assert_eq!(history.forward(), Some("2".to_string()));
        assert_eq!(history.current(), Some(&"2".to_string()));

        // Can't go forward further
        assert_eq!(history.forward(), None);
    }

    #[test]
    fn test_push_truncates_forward_history() {
        let mut history = NavigationHistory::new();

        history.push("0".to_string());
        history.push("1".to_string());
        history.push("2".to_string());

        // Go back to "1"
        history.back();
        assert_eq!(history.current(), Some(&"1".to_string()));
        assert!(history.can_go_forward());

        // Push a new path - should truncate "2"
        history.push("3".to_string());

        assert_eq!(history.len(), 3); // "0", "1", "3"
        assert_eq!(history.current(), Some(&"3".to_string()));
        assert!(!history.can_go_forward()); // "2" was truncated
        assert!(history.can_go_back());
    }

    #[test]
    fn test_max_history_limit() {
        let mut history = NavigationHistory::with_capacity(3);

        history.push("0".to_string());
        history.push("1".to_string());
        history.push("2".to_string());
        history.push("3".to_string()); // Should remove "0"

        assert_eq!(history.len(), 3);
        assert_eq!(history.current(), Some(&"3".to_string()));

        // Go all the way back - should only reach "1" (not "0")
        history.back();
        history.back();
        assert_eq!(history.current(), Some(&"1".to_string()));
        assert!(!history.can_go_back()); // "0" was removed
    }

    #[test]
    fn test_clear_removes_all_history() {
        let mut history = NavigationHistory::new();

        history.push("0".to_string());
        history.push("1".to_string());
        history.push("2".to_string());

        history.clear();

        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        assert!(history.current().is_none());
        assert!(!history.can_go_back());
        assert!(!history.can_go_forward());
    }

    #[test]
    fn test_navigation_preserves_current_index() {
        let mut history = NavigationHistory::new();

        history.push("0".to_string());
        history.push("1".to_string());
        history.push("2".to_string());

        // Navigate back and forth
        history.back();
        assert_eq!(history.current(), Some(&"1".to_string()));

        history.forward();
        assert_eq!(history.current(), Some(&"2".to_string()));

        history.back();
        history.back();
        assert_eq!(history.current(), Some(&"0".to_string()));
    }

    #[test]
    fn test_complex_navigation_scenario() {
        let mut history = NavigationHistory::new();

        // User navigates: 0 -> 0.user -> 0.user.name
        history.push("0".to_string());
        history.push("0.user".to_string());
        history.push("0.user.name".to_string());

        // User goes back twice
        history.back(); // to 0.user
        history.back(); // to 0

        // User navigates to different path
        history.push("1".to_string()); // Truncates forward history

        // History should now be: ["0", "1"]
        assert_eq!(history.len(), 2);
        assert_eq!(history.current(), Some(&"1".to_string()));

        // Can go back to "0" but not forward
        assert!(history.can_go_back());
        assert!(!history.can_go_forward());

        assert_eq!(history.back(), Some("0".to_string()));
        assert!(!history.can_go_back());
        assert!(history.can_go_forward());
    }
}
