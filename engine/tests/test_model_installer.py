import hashlib
import importlib.util
import json
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('installer', Path(__file__).resolve().parents[2] / 'scripts/install_models.py')
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)


class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'models'
        self.model = {'id': 'tiny', 'repository': 'test/model', 'revision': 'a' * 40}
        self.data = {name: name.encode() for name in installer.REQUIRED}
        self.assets = [SimpleNamespace(rfilename=n, size=len(v), lfs={'sha256': hashlib.sha256(v).hexdigest()} if n == 'model.bin' else None) for n, v in self.data.items()]
        self.api = SimpleNamespace(model_info=lambda *a, **k: SimpleNamespace(sha=self.model['revision'], siblings=self.assets))

    def download(self, **kwargs):
        (Path(kwargs['local_dir']) / kwargs['filename']).write_bytes(self.data[kwargs['filename']])

    def test_install_reuse_and_tamper_detection(self):
        installer.install(self.model, self.root, self.api, self.download)
        installer.install(self.model, self.root, None, lambda **k: self.fail('network on reuse'))
        (self.root / 'tiny/model.bin').write_bytes(b'bad')
        with self.assertRaises(ValueError):
            installer.verify(self.root / 'tiny', self.model)

    def test_failed_download_never_publishes_and_can_resume(self):
        def fail(**kwargs):
            raise OSError('offline')
        with self.assertRaises(OSError):
            installer.install(self.model, self.root, self.api, fail)
        self.assertFalse((self.root / 'tiny').exists())
        installer.install(self.model, self.root, self.api, self.download)

    def test_checksum_failure_never_publishes(self):
        self.assets[0].lfs = {'sha256': '0' * 64}
        with self.assertRaises(ValueError):
            installer.install(self.model, self.root, self.api, self.download)
        self.assertFalse((self.root / 'tiny').exists())

    def test_custom_directory_preserved(self):
        (self.root / 'tiny').mkdir(parents=True)
        (self.root / 'tiny/custom').write_text('keep')
        with self.assertRaises(ValueError):
            installer.install(self.model, self.root, self.api, self.download)
        self.assertEqual((self.root / 'tiny/custom').read_text(), 'keep')

    def test_symlink_rejected(self):
        self.root.mkdir()
        (self.root / 'tiny').symlink_to(Path(self.temp.name), target_is_directory=True)
        with self.assertRaises(ValueError):
            installer.install(self.model, self.root, self.api, self.download)


if __name__ == '__main__':
    unittest.main()
