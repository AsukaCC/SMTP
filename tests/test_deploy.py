"""部署状态机测试；不执行 Docker 或修改用户实例。"""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('deploy', Path(__file__).parents[1] / 'scripts/deploy.py')
deploy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(deploy)


def release(number):
    return {'version': number, 'schema': 'a' * 64, 'image': 'sha256:' + 'b' * 64}


class FakeDeployment(deploy.Deployment):
    def __init__(self, path):
        super().__init__(path)
        self.events = []
        self.fail = set()

    def compose(self, target, *args, **kwargs):
        self.events.append(('compose', target['version'], *args))

    def backup(self, target):
        self.events.append(('backup', target['version']))
        if 'backup' in self.fail:
            raise OSError('backup failure')
        return '/private/backup.dump'

    def start(self, target, install=False):
        self.events.append(('start', target['version']))
        if target['version'] in self.fail:
            raise subprocess.CalledProcessError(1, ['docker'])


class DeployTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / '.env').touch()
        self.app = FakeDeployment(self.root)
        for number in ('0.1.0', '0.1.1', '0.1.2'):
            (self.app.meta / 'releases' / number).mkdir(parents=True)
        self.state = {'directory': str(self.root.resolve()), 'project': self.app.project,
                      'current': release('0.1.1'), 'previous': release('0.1.0'),
                      'pending': {'target': release('0.1.2'), 'original': release('0.1.1')}}
        self.run = patch.object(deploy, 'run', return_value=subprocess.CompletedProcess([], 0, stdout='')).start()
        self.addCleanup(patch.stopall)

    def test_success_backs_up_before_start_and_retains_two(self):
        self.app.resume(self.state)
        self.assertEqual(self.app.events[:3], [('compose', '0.1.1', 'stop', 'app'), ('backup', '0.1.1'), ('start', '0.1.2')])
        self.assertEqual(self.app.state()['current']['version'], '0.1.2')
        self.assertEqual(sorted(p.name for p in (self.app.meta / 'releases').iterdir()), ['0.1.1', '0.1.2'])

    def test_health_failure_restores_old_version(self):
        self.app.fail.add('0.1.2')
        with self.assertRaisesRegex(ValueError, '已自动恢复'):
            self.app.resume(self.state)
        self.assertEqual(self.app.events[-1], ('start', '0.1.1'))
        self.assertEqual(self.app.state()['current']['version'], '0.1.1')
        self.assertIsNone(self.app.state()['pending'])
        self.assertEqual(self.app.state()['failures'][0]['backup'], '/private/backup.dump')

    def test_backup_failure_never_starts_target(self):
        self.app.fail.add('backup')
        with self.assertRaises(ValueError):
            self.app.resume(self.state)
        self.assertNotIn(('start', '0.1.2'), self.app.events)
        self.assertEqual(self.app.events[-1], ('start', '0.1.1'))

    def test_missing_image_does_not_stop_service(self):
        self.run.side_effect = subprocess.CalledProcessError(1, ['inspect'])
        with self.assertRaises(subprocess.CalledProcessError):
            self.app.resume(self.state)
        self.assertEqual(self.app.events, [])

    def test_double_failure_keeps_pending(self):
        self.app.save(self.state)
        self.app.fail.update(['0.1.1', '0.1.2'])
        with self.assertRaisesRegex(ValueError, '新旧版本启动均失败'):
            self.app.resume(self.state)
        self.assertTrue(self.app.state()['pending'])

    def test_duplicate_and_manual_install_are_rejected(self):
        self.app.save(self.state)
        with self.assertRaisesRegex(ValueError, '已安装'):
            self.app.change('install', self.root)
        (self.app.meta / 'state.json').unlink()
        self.run.side_effect = [subprocess.CompletedProcess([], 0, stdout=''), subprocess.CompletedProcess([], 0, stdout='manual-container'), subprocess.CompletedProcess([], 0, stdout='')]
        with self.assertRaisesRegex(ValueError, '不能重复安装'):
            self.app.change('install', self.root)

    def test_lock_is_directory_scoped_and_released(self):
        with self.app.lock():
            with self.assertRaisesRegex(ValueError, '已有部署操作'):
                with FakeDeployment(self.root).lock():
                    pass
            with FakeDeployment(self.root / 'other').lock():
                pass
        with self.app.lock():
            pass

    def test_incompatible_schema_rejected_before_preparation(self):
        self.state['pending'] = None
        self.app.save(self.state)
        candidate = {**release('0.1.2'), 'schema': 'c' * 64}
        with patch.object(deploy, 'read_package', return_value=candidate):
            with self.assertRaisesRegex(ValueError, '数据库迁移指纹变化'):
                self.app.change('update', self.root)
        self.assertEqual(self.app.events, [])


if __name__ == '__main__':
    unittest.main()
