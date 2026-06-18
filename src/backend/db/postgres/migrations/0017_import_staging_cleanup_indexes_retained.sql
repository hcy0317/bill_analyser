-- Compatibility migration for deployments that already recorded version 17.
-- Version 0016 creates the import staging cleanup and foreign-key helper
-- indexes. They are intentionally retained because deleting import sessions
-- without these indexes can degrade from sub-second cleanup to minute-level
-- foreign-key scans on large preview sessions.

