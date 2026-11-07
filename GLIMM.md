# Glimm в GPUI

Локальная копия [longbridge/gpui-fast](https://github.com/longbridge/gpui-fast) находится в `gpui-fast/`, базовый коммит `4a4a89a`. Начальные `Cargo.toml` и `src/` внешнего проекта не менялись.

## Запуск

macOS / Linux:

```sh
./run-glimm.sh
```

Windows (PowerShell, установленный Rust и Windows SDK):

```powershell
.\run-glimm.ps1
```

В окне — девять **исходных примеров glimm.dev**, а не общий интерфейс с разными пресетами:

| № | Пример | Что воспроизведено |
|---|---|---|
| 0 | Generate data | Исходный промпт, таблица сделок, Generate / Stop / Clear, ожидание работы 2600 мс |
| 1 | Teammate joins | Аватары, вступление Tom Becker, всплывающая карточка после прохода эффекта |
| 2 | Invoice paid | Исходный каркас приложения, синее свечение и точки, уведомление об invoice #0042 |
| 3 | Upgrade to Pro | Карточка с оригинальным оформлением и ценой $12, состояние Free → Pro |
| 4 | Usage reset | Обратный отсчёт, исходный композер, снятие ограничения |
| 5 | Publish | Оригинальная страница редактора, ожидание публикации, страница результата и Back |
| 6 | Autopilot | Исходный переключатель Off → On и графика включённого состояния |
| 7 | Theme | Исходная сетка карточек, светлая и тёмная темы, разные палитры для двух направлений |
| 8 | Tasks | Пять отдельных задач, все 32 комбинации, эффект и уведомление при завершении всех |

Сцены рисуются **нативными quads, текстом, изображениями и элементами управления GPUI**. HTML, JavaScript и WebView в приложении не запускаются. Inter, фотографии, SVG-иконки и декоративные градиенты взяты из оригинала. Страницы целиком не подменены картинками. Образцы имеют исходный размер embedded viewport **634 × 476 логических пикселей**; панель настроек находится снаружи.

## Границы и точки действия

У таблицы верхние панели высотой 44 + 40 + 32 px и композер **не входят в эффект**. `glimm_scope` охватывает только viewport таблицы `(0,116,634,360)`. Источник деформации — центр композера относительно этого viewport: **`[317,293]`**, а не центр окна или кнопки Generate.

У остальных примеров сохранены собственные границы и точки действия: вся группа аватаров у Join, центр сцены у Upgrade, Publish / Back, переключатели, последняя нажатая строка задачи и композер usage reset. Большинство исходных scope занимают весь embedded viewport — локальность задаётся ещё и кривизной, span, fade и точкой запуска. Например, уведомление invoice и его декоративные overlays нарисованы **за пределами** деформируемого каркаса приложения.

Внешняя кнопка **Show point of action** показывает фактический `at`. **Origin: original element / custom options** переключает привязку к исходному элементу и координатам из настроек.

## Кастомизация

В панели: Scope / Band, четыре направления, палитры, sweep/gather timing, интенсивность деформации, curve, span, band tightness, brightness, swell, reduced motion, pause/resume, fast-forward, отмена и копирование JSON.

Все параметры доступны через частичный JSON:

```sh
./run-glimm.sh --config ../glimm-settings.json
./run-glimm.sh --config ../glimm-band-settings.json
./run-glimm.sh --demo 7
```

Пути CLI разрешаются относительно `gpui-fast/`. Публичный API и пример интеграции со **своим нативным содержимым**: [`gpui-fast/docs/glimm.md`](gpui-fast/docs/glimm.md).

## Сравнение с оригиналом

```sh
# Только исходная сцена, без внешней панели
./run-glimm.sh --scene-only --demo 0

# Кадры исходного и конечного состояний
./run-glimm.sh --scene-only --demo 0 --state initial --capture ../glimm-native-sheet-before.png
./run-glimm.sh --scene-only --demo 0 --state final --capture ../glimm-native-sheet-after.png

# Время отсчитывается от действия; таблица ожидает работу первые 2600 мс
./run-glimm.sh --scene-only --demo 0 --time 3290 --capture ../glimm-native-sheet-transition.png
./run-glimm.sh --scene-only --demo 0 --gather --time 1600 --capture ../glimm-native-sheet-gather.png
```

Скрипты в корне:

- `export-glimm-scenes.mjs` — измерения оригинального DOM и декоративных ресурсов; нужен Chrome с HTML-in-Canvas. Само нативное приложение от браузера не зависит.
- `smoke-glimm.mjs` — 31 нативный захват: before / after / переход для девяти сцен, gather, Band, пользовательские настройки и полное окно.
- `compare-glimm-scenes.py` — сравнение с исходными embedded-сценами и проверка неподвижных областей таблицы. Запуск: `uv run --with pillow --with numpy python compare-glimm-scenes.py`.
- `verify-glimm.mjs` — оригинальный WebGL oracle для общей GPU-тестовой сцены; тот же fixture проверяется в Metal, WGPU и D3D11.
- `compare-glimm-platforms.py linux|windows` — сравнения отдельных OS-захватов с оригиналом и проверка изоляции Sheet. `smoke-glimm.mjs` принимает `GLIMM_DEMO_BIN`, `GLIMM_CAPTURE_PREFIX` и `GLIMM_SMOKE_RESULTS`, чтобы не перезаписывать кадры другой платформы.

`glimm-compare-<demo>-before.png` и `glimm-compare-<demo>-after.png`: слева сайт, справа GPUI. Численные результаты — `glimm-scene-comparison.json`; проверка таблицы требует **нулевой разницы** у верхних панелей, текста промпта и кнопки Add между idle и gather. Это отдельная проверка от shader golden.

Разные растеризаторы Chrome/CoreText, формы скруглений и ядра размытия теней дают небольшие пиксельные отличия; побитовая идентичность кадров не заявляется. Дополнительные появления Invoice и карточки Join используют исходную пружину и тайминги. Кратковременный CSS blur этих overlays не воспроизведён точно, а изменение штриховки колец аватаров заменено crossfade исходных состояний.

## Проверка

Из `gpui-fast/`:

```sh
# macOS
SDKROOT="$(xcrun --sdk macosx --show-sdk-path)" cargo test -p gpui -p gpui_apple --lib
# Linux
cargo test -p gpui_wgpu --features test-support --lib fast::glimm -- --nocapture
# Windows
cargo test -p gpui_windows --features test-support --lib fast::glimm -- --nocapture
# Общие тесты девяти сцен на каждой ОС
cargo test -p gpui_perf --example glimm --features gpui_platform_fast/test-support
script/check-upstream
git diff --check
```

Нативные проверки выполняются в Tart, не в Docker/Wine: Ubuntu ARM64 + Vulkan/lavapipe и Windows 11 ARM64 + D3D11 WARP. Windows запускает x86_64 GNU-сборку через штатную x64-эмуляцию, а не ARM64-бинарник. Linux-кадры сняты в X11/Xvfb; Wayland-хук захвата проверен сборкой, но не отдельным compositor runtime.

В общем Linux WGPU-наборе остаются четыре старых падения тестов растеризации path tiles на lavapipe. Они воспроизведены на чистом базовом коммите; все тесты Glimm проходят.

Результаты, 62 OS-захвата, shader parity и точные границы проверки собраны в [GLIMM-PLATFORMS.md](GLIMM-PLATFORMS.md).

## Ограничения

- Рендереры: macOS / Metal, Linux / WGPU и Windows / Direct3D 11. Linux проверяется в Tart на Vulkan/lavapipe, Windows — в Tart на программном D3D11 WARP. Это не замер производительности реальной видеокарты. FreeBSD, браузерные и мобильные WGPU-цели отдельно не проверялись.
- Это локальные интерактивные примеры без настоящих платежей, аккаунтов, публикации и сетевого сервиса. Второстепенные кнопки, не имеющие действий в оригинале, не подключают выдуманный backend.
- Новый `play` заменяет текущий запуск; очереди нет. Перезапуск не снимает snapshot уже деформированного framebuffer.
- Экспериментальные npm `Mesh` / 3D `NameDrop` не портированы: сайт использует имя `NameDropScope` для обычного `GlimmScope`.
- Коммиты и push не выполнялись.
