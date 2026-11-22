import unittest

from check_release_version import minimum_version


class ReleaseVersionTest(unittest.TestCase):
    def test_breaking_commit_after_patch_bump_requires_major(self):
        self.assertEqual(
            minimum_version(
                (0, 2, 4),
                ["fix: improve cache writes", "chore: release v0.2.5", "fix(deps)!: change types"],
            ),
            (1, 0, 0),
        )

    def test_breaking_footer_takes_precedence_over_subject(self):
        self.assertEqual(
            minimum_version(
                (1, 4, 1),
                ["chore: update types\n\nBREAKING CHANGE: revise the public entry type"],
            ),
            (2, 0, 0),
        )

    def test_feature_after_patch_bump_requires_minor(self):
        self.assertEqual(
            minimum_version((1, 4, 1), ["chore: release v1.4.2", "feat: add lookup"]),
            (1, 5, 0),
        )

    def test_additional_fix_fits_existing_patch(self):
        self.assertEqual(
            minimum_version((1, 4, 1), ["chore: release v1.4.2", "fix: restore lookup"]),
            (1, 4, 2),
        )
