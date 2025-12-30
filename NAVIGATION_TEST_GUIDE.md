# Navigation History Testing Guide

## Phase 1: Navigation History - Manual Testing Checklist

This guide covers testing the back/forward navigation history feature implemented for issue #9.

### Prerequisites
1. Build the project: `cargo build --release`
2. Have a JSON file with nested structure ready for testing (or use the example below)

### Test JSON File

Create a file called `test_navigation.json`:

```json
{
  "users": [
    {
      "id": 1,
      "name": "Alice",
      "email": "alice@example.com",
      "address": {
        "street": "123 Main St",
        "city": "Springfield",
        "zipcode": "12345"
      }
    },
    {
      "id": 2,
      "name": "Bob",
      "email": "bob@example.com",
      "address": {
        "street": "456 Oak Ave",
        "city": "Shelbyville",
        "zipcode": "67890"
      }
    }
  ],
  "settings": {
    "theme": "dark",
    "language": "en",
    "notifications": {
      "email": true,
      "push": false
    }
  }
}
```

---

## Test Cases

### Test 1: Basic Path Selection History
**Objective**: Verify that path selections are tracked in navigation history

**Steps**:
1. Open `test_navigation.json` in Thoth
2. Click on `users` to select it
3. Click on `users[0]` to select it
4. Click on `users[0].name` to select it
5. Click on `settings` to select it
6. Click on `settings.notifications` to select it

**Expected Result**: Each selection should be tracked (verified in next tests)

---

### Test 2: Keyboard Back Navigation (Cmd+[ / Ctrl+[)
**Objective**: Verify keyboard shortcut for going back through history

**Steps**:
1. Continue from Test 1 (currently at `settings.notifications`)
2. Press `Cmd+[` (Mac) or `Ctrl+[` (Linux/Windows)
3. Press `Cmd+[` again
4. Press `Cmd+[` again
5. Press `Cmd+[` again
6. Press `Cmd+[` again

**Expected Results**:
- After 1st press: Selection moves to `settings`
- After 2nd press: Selection moves to `users[0].name`
- After 3rd press: Selection moves to `users[0]`
- After 4th press: Selection moves to `users`
- After 5th press: No change (at beginning of history)

---

### Test 3: Keyboard Forward Navigation (Cmd+] / Ctrl+])
**Objective**: Verify keyboard shortcut for going forward through history

**Steps**:
1. Continue from Test 2 (currently at `users`)
2. Press `Cmd+]` (Mac) or `Ctrl+]` (Linux/Windows)
3. Press `Cmd+]` again
4. Press `Cmd+]` again
5. Press `Cmd+]` again
6. Press `Cmd+]` again

**Expected Results**:
- After 1st press: Selection moves to `users[0]`
- After 2nd press: Selection moves to `users[0].name`
- After 3rd press: Selection moves to `settings`
- After 4th press: Selection moves to `settings.notifications`
- After 5th press: No change (at end of history)

---

### Test 4: Mouse Button Back Navigation
**Objective**: Verify mouse button 4 (Extra1) triggers back navigation

**Prerequisites**: Requires a mouse with extra buttons (typically thumb buttons on the side)

**Steps**:
1. Navigate to several paths (e.g., follow Test 1 steps)
2. Click the "back" mouse button (typically button 4 / thumb button)

**Expected Result**: Should navigate back to previous selection (same as Cmd+[)

---

### Test 5: Mouse Button Forward Navigation
**Objective**: Verify mouse button 5 (Extra2) triggers forward navigation

**Prerequisites**: Requires a mouse with extra buttons

**Steps**:
1. Navigate back through history using back button or Cmd+[
2. Click the "forward" mouse button (typically button 5 / thumb button)

**Expected Result**: Should navigate forward to next selection (same as Cmd+])

---

### Test 6: History Truncation on New Path
**Objective**: Verify that forward history is cleared when navigating to a new path

**Steps**:
1. Navigate to several paths: `users` → `users[0]` → `users[0].name` → `settings`
2. Go back twice: Should be at `users[0]`
3. Now navigate to a NEW path by clicking `settings.theme`
4. Try to go forward with `Cmd+]`

**Expected Results**:
- After step 3: Selection is at `settings.theme`
- After step 4: No change (forward history was truncated when we navigated to `settings.theme`)

---

### Test 7: Navigation History Size Setting
**Objective**: Verify the navigation history size can be configured in settings

**Steps**:
1. Open Settings (Cmd+, or menu)
2. Go to Performance tab
3. Locate "Navigation history size" slider
4. Verify default value is 100
5. Adjust slider to 50
6. Verify range is 10-1000
7. Click Apply to save

**Expected Results**:
- Slider should exist in Performance tab under "Navigation" section
- Default value: 100
- Min value: 10
- Max value: 1000
- Setting should persist when reopening settings

---

### Test 8: History Limit Enforcement
**Objective**: Verify that history is limited to configured size

**Steps**:
1. Set navigation history size to 10 in settings
2. Navigate through more than 10 different paths
3. Try to go back 11 times

**Expected Results**:
- Can only go back through last 10 selections
- Oldest selections are discarded when limit is reached

---

### Test 9: Search Navigation History
**Objective**: Verify that search results integrate with navigation history

**Steps**:
1. Open `test_navigation.json`
2. Open search (Cmd+F)
3. Search for "email"
4. Press Enter to go to first match (`users[0].email`)
5. Press Cmd+G or F3 to go to next match (`users[1].email`)
6. Press Cmd+G again to go to third match (`settings.notifications.email`)
7. Close search
8. Press Cmd+[ to go back

**Expected Results**:
- Search navigation should be tracked in history
- Going back should return to `users[1].email`
- Going back again should return to `users[0].email`

---

### Test 10: Multi-File History Isolation
**Objective**: Verify each file has independent navigation history

**Steps**:
1. Open `test_navigation.json`
2. Navigate to `users[0].name`
3. Open a different JSON file
4. Navigate to some path in the new file
5. Switch back to `test_navigation.json` (Recent Files)
6. Press Cmd+[ to go back

**Expected Results**:
- Each file should maintain its own navigation history
- Switching files should not affect individual file histories
- Back navigation in original file should work from where you left off

---

## Regression Testing

### Existing Features to Verify Still Work

1. **Keyboard Navigation**: Arrow keys, Home, End still navigate properly
2. **Search Navigation**: Cmd+F, Next/Previous match still work
3. **Expand/Collapse**: Enter, Space still expand/collapse nodes
4. **Recent Files**: Still tracks recently opened files
5. **Scroll to Selection**: Selected items still scroll into view

---

## Known Limitations & Edge Cases

1. **Empty History**: Navigation commands do nothing when history is empty
2. **Same Path**: Clicking the same path multiple times doesn't duplicate in history
3. **Session Persistence**: History is NOT persisted - clears when app closes (by design)
4. **Per-File History**: Each file has independent history (not global)

---

## Automated Test Coverage

Unit tests exist in `src/state/tests.rs` covering:
- ✅ Basic push/back/forward operations
- ✅ History limits (max_history)
- ✅ Forward history truncation
- ✅ Empty history edge cases
- ✅ Complex navigation scenarios
- ✅ Same path deduplication

Run tests with:
```bash
cargo test navigation_history
```

---

## Success Criteria

Phase 1 navigation history is complete when:
- [x] Navigation history data structure implemented
- [x] Path selection changes tracked automatically
- [x] Keyboard shortcuts (Cmd+[ / Cmd+]) work
- [x] Mouse buttons (4/5) work
- [x] History size configurable in settings
- [x] History limit enforced correctly
- [x] Forward history truncates on new navigation
- [ ] All manual tests pass
- [x] Unit tests pass with 100% coverage

---

## Next Phase

Once Phase 1 testing is complete and all issues resolved, proceed to:
**Phase 2: Bookmarks System** - Allow users to bookmark specific JSON paths
