"""Collect native OS evidence; fail rather than silently report incomplete runs."""
import json
import re
from pathlib import Path


def results(path):
    text = Path(path).read_text()
    matches = re.findall(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored', text)
    if not matches:
        raise ValueError(f'No completed test run: {path}')
    return [{'status': status, 'passed': int(passed), 'failed': int(failed), 'ignored': int(ignored)} for status, passed, failed, ignored in matches]


def parity(path):
    match = re.search(r'WebGL/[^ ]+ parity: mean ([\d.]+), max (\d+)', Path(path).read_text())
    if match is None:
        raise ValueError(f'No shader comparison: {path}')
    return float(match[1]), int(match[2])


def smoke(path):
    rows = json.loads(Path(path).read_text())
    if len(rows) != 31 or not all(row['ok'] and Path(row['file']).is_file() and row['bytes'] > 0 for row in rows):
        raise ValueError(f'Incomplete native captures: {path}')
    return len(rows)


def failures(path):
    return sorted(set(re.findall(r'^    (fast::layers::tests::\S+)$', Path(path).read_text(), re.MULTILINE)))


platforms = []
isolation = []
for os_name, renderer, environment, gpu_log, demo_log, smoke_log in [
    ('Linux', 'WGPU / Vulkan', 'Ubuntu ARM64 / Tart / lavapipe / X11', 'glimm-tart-linux-glimm-tests.log', 'glimm-tart-linux-clippy.log', 'glimm-linux-smoke.json'),
    ('Windows', 'Direct3D 11 / WARP', 'Windows 11 ARM64 / Tart / x64 emulation', 'glimm-tart-windows-tests.log', 'glimm-tart-windows-demo-tests.log', 'glimm-windows-smoke.json'),
]:
    gpu = results(gpu_log)[-1]
    demo = results(demo_log)[-1]
    if gpu['failed'] or demo['failed']:
        raise ValueError(f'{os_name} tests failed')
    mean, maximum = parity(gpu_log)
    comparison = json.loads(Path(f'glimm-{os_name.lower()}-comparison.json').read_text())
    for region in comparison['sheet_isolation']:
        if region['changed_pixels'] or region['max_channel_delta']:
            raise ValueError(f'{os_name}: pixels outside the table changed')
        isolation.append({'os': os_name, **region})
    platforms.append({'os': os_name, 'renderer': renderer, 'environment': environment, 'gpu_tests': gpu['passed'], 'scene_tests': demo['passed'], 'captures': smoke(smoke_log), 'shader_mean_255': mean, 'shader_max_255': maximum})

baseline_failures = failures('glimm-tart-linux-baseline-tests.log')
current_failures = failures('glimm-tart-linux-final-tests.log')
if len(current_failures) != 4 or current_failures != baseline_failures:
    raise ValueError('Linux failures do not match the clean base checkout')
host = results('glimm-host-core-tests.log') + results('glimm-host-backend-full-tests.log') + results('glimm-host-demo-tests.log')
if any(run['failed'] for run in host):
    raise ValueError('Host regressions failed')
report = {
    'platforms': platforms,
    'sheet_isolation': isolation,
    'host_regressions': {'passed': sum(run['passed'] for run in host), 'ignored': sum(run['ignored'] for run in host)},
    'linux_full_suite': results('glimm-tart-linux-final-tests.log')[-1],
    'linux_clean_base_suite': results('glimm-tart-linux-baseline-tests.log')[-1],
    'linux_existing_failures': current_failures,
    'limitations': ['Guest software adapters do not establish physical GPU performance.', 'Linux window captures exercise X11; the Wayland capture hook has compile checks only.', 'Windows runs an x86_64 GNU executable through Windows ARM64 x64 emulation.', 'Native font, corner and shadow rasterization is not pixel-identical to Chrome.'],
}
Path('glimm-platform-verification.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report, indent=2))
