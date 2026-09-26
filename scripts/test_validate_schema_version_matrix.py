#!/usr/bin/env python3
"""
Tests for validate_schema_version_matrix.py

Tests the mismatch detection case and parsing logic.
Can be run standalone or via pytest.
"""

import sys
import tempfile
from pathlib import Path

# Import the module under test
from validate_schema_version_matrix import (
    extract_schema_version,
    extract_latest_version_from_matrix,
    validate_schema_version,
)


def test_extract_schema_version_valid():
    """Test extracting a valid schema version from lib.rs content."""
    with tempfile.TemporaryDirectory() as tmpdir:
        lib_rs = Path(tmpdir) / "lib.rs"
        lib_rs.write_text(
            """
pub const SCHEMA_VERSION: u32 = 9;
        """
        )
        version = extract_schema_version(lib_rs)
        assert version == 9, f"Expected 9, got {version}"
        print("✓ test_extract_schema_version_valid passed")


def test_extract_schema_version_with_whitespace():
    """Test extracting version with various whitespace."""
    with tempfile.TemporaryDirectory() as tmpdir:
        lib_rs = Path(tmpdir) / "lib.rs"
        lib_rs.write_text(
            """
pub   const   SCHEMA_VERSION   :   u32   =   7   ;
        """
        )
        version = extract_schema_version(lib_rs)
        assert version == 7, f"Expected 7, got {version}"
        print("✓ test_extract_schema_version_with_whitespace passed")


def test_extract_schema_version_not_found():
    """Test error handling when SCHEMA_VERSION is missing."""
    with tempfile.TemporaryDirectory() as tmpdir:
        lib_rs = Path(tmpdir) / "lib.rs"
        lib_rs.write_text("// No schema version here")
        try:
            extract_schema_version(lib_rs)
            assert False, "Should have raised ValueError"
        except ValueError as e:
            assert "Could not find" in str(e)
            print("✓ test_extract_schema_version_not_found passed")


def test_extract_schema_version_file_not_found():
    """Test error handling when lib.rs file doesn't exist."""
    try:
        extract_schema_version(Path("/nonexistent/lib.rs"))
        assert False, "Should have raised FileNotFoundError"
    except FileNotFoundError:
        print("✓ test_extract_schema_version_file_not_found passed")


def test_extract_latest_version_from_matrix_valid():
    """Test extracting the latest version from matrix markdown."""
    with tempfile.TemporaryDirectory() as tmpdir:
        matrix = Path(tmpdir) / "matrix.md"
        matrix.write_text(
            """
# Schema Version Matrix

| Schema Version | Status | ... |
| 1 | Deprecated | ... |
| 9 | **Current** | ... |
        """
        )
        version = extract_latest_version_from_matrix(matrix)
        assert version == 9, f"Expected 9, got {version}"
        print("✓ test_extract_latest_version_from_matrix_valid passed")


def test_extract_latest_version_from_matrix_whitespace():
    """Test extracting version with variable whitespace in markdown."""
    with tempfile.TemporaryDirectory() as tmpdir:
        matrix = Path(tmpdir) / "matrix.md"
        matrix.write_text(
            """
| 6 |  **Current**  |  Features  |
        """
        )
        version = extract_latest_version_from_matrix(matrix)
        assert version == 6, f"Expected 6, got {version}"
        print("✓ test_extract_latest_version_from_matrix_whitespace passed")


def test_extract_latest_version_from_matrix_not_found():
    """Test error handling when **Current** marker is missing."""
    with tempfile.TemporaryDirectory() as tmpdir:
        matrix = Path(tmpdir) / "matrix.md"
        matrix.write_text(
            """
| Schema Version | Status |
| 1 | Deprecated |
| 9 | Future |
        """
        )
        try:
            extract_latest_version_from_matrix(matrix)
            assert False, "Should have raised ValueError"
        except ValueError as e:
            assert "**Current**" in str(e)
            print("✓ test_extract_latest_version_from_matrix_not_found passed")


def test_validate_schema_version_match():
    """Test validation when versions match."""
    with tempfile.TemporaryDirectory() as tmpdir:
        lib_rs = Path(tmpdir) / "lib.rs"
        matrix = Path(tmpdir) / "matrix.md"

        lib_rs.write_text("pub const SCHEMA_VERSION: u32 = 9;")
        matrix.write_text("| 9 | **Current** | Features |")

        is_valid, message = validate_schema_version(lib_rs, matrix)
        assert is_valid, f"Validation should succeed: {message}"
        assert "✓" in message
        print("✓ test_validate_schema_version_match passed")


def test_validate_schema_version_mismatch():
    """Test validation failure when versions don't match (the key test case)."""
    with tempfile.TemporaryDirectory() as tmpdir:
        lib_rs = Path(tmpdir) / "lib.rs"
        matrix = Path(tmpdir) / "matrix.md"

        # lib.rs says version 10, but matrix says version 9 is current
        lib_rs.write_text("pub const SCHEMA_VERSION: u32 = 10;")
        matrix.write_text("| 9 | **Current** | Features |")

        is_valid, message = validate_schema_version(lib_rs, matrix)
        assert not is_valid, f"Validation should fail: {message}"
        assert "✗" in message
        assert "mismatch" in message.lower()
        assert "10" in message
        assert "9" in message
        print("✓ test_validate_schema_version_mismatch passed (THIS IS THE KEY TEST)")


def test_validate_schema_version_multiple_entries():
    """Test when matrix has multiple version entries."""
    with tempfile.TemporaryDirectory() as tmpdir:
        lib_rs = Path(tmpdir) / "lib.rs"
        matrix = Path(tmpdir) / "matrix.md"

        lib_rs.write_text("pub const SCHEMA_VERSION: u32 = 8;")
        matrix.write_text(
            """
# Schema Version Matrix

| Schema Version | Status |
| 6 | Deprecated |
| 7 | Deprecated |
| 8 | **Current** |
| 9 | Planned |
        """
        )

        is_valid, message = validate_schema_version(lib_rs, matrix)
        assert is_valid, f"Validation should succeed: {message}"
        print("✓ test_validate_schema_version_multiple_entries passed")


def run_all_tests():
    """Run all tests and report results."""
    tests = [
        test_extract_schema_version_valid,
        test_extract_schema_version_with_whitespace,
        test_extract_schema_version_not_found,
        test_extract_schema_version_file_not_found,
        test_extract_latest_version_from_matrix_valid,
        test_extract_latest_version_from_matrix_whitespace,
        test_extract_latest_version_from_matrix_not_found,
        test_validate_schema_version_match,
        test_validate_schema_version_mismatch,
        test_validate_schema_version_multiple_entries,
    ]

    passed = 0
    failed = 0

    for test in tests:
        try:
            test()
            passed += 1
        except Exception as e:
            print(f"✗ {test.__name__} failed: {e}")
            failed += 1

    print(f"\n{passed} passed, {failed} failed")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(run_all_tests())
