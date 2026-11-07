# Glimm: Linux и Windows

Проверка от 7 ноября 2026. Код находится в локальном `gpui-fast/`, базовый коммит `4a4a89a`; изменений в upstream и опубликованных crates нет.

## Что перенесено

- **Linux / WGPU:** WGSL-шейдеры, изолированная композиция областей, cropped old/live snapshots и mipmaps, правильный premultiplied-alpha raster, повторное использование текстур, resize и освобождение ресурсов в idle.
- **Windows / Direct3D 11:** HLSL-шейдеры, тот же scoped-композитор, mipmaps, прозрачность, кэш ресурсов и сброс при смене устройства.
- Общий API контроллера, JSON, палитры, четыре направления, Band/Content и все девять исходных нативных сцен сохранены. Sheet деформирует таблицу, а не всю страницу. Вложенные области, соседние области и overlays проверяются общими GPU-тестами.
- Linux получил test-support захват через собственный WGPU device и sprite atlas окна для X11/Wayland. В CI добавлены проверки всех трёх рендереров. Для Windows добавлен `run-glimm.ps1`.

## Реальные прогоны

Обе гостевые ОС запущены в **Tart**, не в Docker/Wine.

| Проверка | Linux | Windows |
| --- | ---: | ---: |
| GPU-тесты Glimm | 13/13 | 12/12 |
| Тесты девяти сцен и действий | 6/6 | 6/6 |
| Нативные PNG-захваты | 31/31 | 31/31 |
| Средняя RGB-ошибка шейдерного fixture против оригинального WebGL, /255 | 0.209757 | 0.162974 |
| Максимальная ошибка канала того же fixture, /255 | 2 | 2 |
| Изменённые пиксели в проверенных областях вне таблицы Sheet | 0 | 0 |

Linux: Ubuntu 24.04 ARM64, Vulkan/lavapipe; окна и PNG проверены через X11/Xvfb при масштабе 2. Windows: Windows 11 ARM64, Direct3D 11 WARP; выполняется x86_64 GNU-сборка через штатную x64-эмуляцию. Сам HLSL действительно скомпилирован и выполнен внутри Windows, а не только включён в Rust-бинарник.

На macOS повторный набор `gpui`, `gpui_apple`, `gpui_wgpu` и тесты примера дал **979 passed, 5 ignored**. Общий Metal GPU-набор: 11/11, WGPU на Metal: 13/13. Строгий Clippy, целевой rustfmt, `script/check-upstream` и `git diff --check` проверены. Отдельно проверены Linux-сборки только с X11 и только с Wayland. В Linux X11-only конфигурации есть два старых dead-code предупреждения в upstream platform helpers; это не ошибки сборки.

## Что не скрыто

Полный WGPU lib-набор на Linux/lavapipe **не полностью зелёный**:

- Текущий код: **99 passed, 4 failed, 1 ignored**.
- Чистый базовый коммит в той же Tart VM: **86 passed, 4 failed, 1 ignored**.
- Набор четырёх падений совпадает; 13 добавленных тестов Glimm проходят.

Существующие падения относятся к `fast::layers::tests`:

1. `a_rasterized_tile_equals_the_same_content_drawn_directly`
2. `paths_composite_exactly_at_even_translations`
3. `resize_releases_every_tile`
4. `tiles_of_negative_coordinates_rasterize_like_positive_ones`

Логи: `glimm-tart-linux-final-tests.log` и `glimm-tart-linux-baseline-tests.log`. Исходный baseline для повторения сохранён в `glimm-linux-baseline/`.

## Артефакты

- [`glimm-platform-verification.json`](glimm-platform-verification.json) — сводка, проверяемая скриптом `glimm-platform-report.py`.
- [`glimm-linux-smoke.json`](glimm-linux-smoke.json), [`glimm-windows-smoke.json`](glimm-windows-smoke.json) — все 62 результата захватов.
- `glimm-linux-compare-*.png`, `glimm-windows-compare-*.png` — оригинальный браузер слева, нативный GPUI справа для всех девяти сцен.
- `glimm-linux-comparison.json`, `glimm-windows-comparison.json` — layout-метрики и pixel isolation таблицы.
- `glimm-linux-gpu/`, `glimm-windows-gpu/`, `glimm-macos-gpu/` — отдельные GPU fixture-кадры и uniforms.
- `glimm-tart-windows-tests.log`, `glimm-tart-windows-demo-tests.log`, `glimm-tart-linux-glimm-tests.log` — runtime-логи.
- `glimm-host-core-tests.log`, `glimm-host-backend-full-tests.log`, `glimm-host-demo-tests.log` — host-регрессии.

Для повторной проверки уже снятых кадров:

```sh
python3 compare-glimm-platforms.py linux
python3 compare-glimm-platforms.py windows
python3 glimm-platform-report.py
```

## Запуск приложения

macOS / Linux:

```sh
./run-glimm.sh
```

Windows PowerShell с установленными Rust и Windows SDK:

```powershell
.\run-glimm.ps1
```

Tart VM `glimm-linux` сохранена в обычном пользовательском Tart storage. Windows VM `glimm-windows` хранится отдельно в `glimm-tart-windows-vms/` и использует проверенный Windows-preview fork в `glimm-tart-windows-runtime/`. Системный Tart и чужие VM не заменялись и не удалялись. В Windows локально скопированы HLSL-файлы для абсолютных debug-путей cross-compiled сборки: VirtioFS не поддерживает нужный Windows canonicalize; это подготовка VM, не изменение рендерера приложения.

## Границы проверки

- Это функциональная проверка программных гостевых GPU, не benchmark физических Linux/Windows видеокарт.
- Wayland-хук прошёл сборку, но отдельного runtime-прогона в Wayland compositor не было.
- FreeBSD, браузерные и мобильные WGPU-цели не проверялись.
- Нативные font antialiasing, corner geometry и shadow kernels отличаются от Chrome. Побитовая идентичность полных UI-кадров не заявляется; числовое сравнение шейдеров относится к описанному fixture.
- Коммиты и push не выполнялись.
