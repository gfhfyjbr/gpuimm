import UIKit
import Metal
import QuartzCore
import SwiftUI

@MainActor
private final class FrameTarget: NSObject {
    weak var view: GpuimmView?
    @objc func frame(_ link: CADisplayLink) { view?.renderFrame() }
}

/// An embeddable GPUI view. Put native controls in its parent, above this view.
/// UIKit owns focus and hit testing; no touch from a sibling control enters Rust.
final class GpuimmView: UIView {
    override class var layerClass: AnyClass { CAMetalLayer.self }
    private(set) var handle: UInt64 = 0
    private var displayLink: CADisplayLink?
    private let target = FrameTarget()
    private var touchIDs: [ObjectIdentifier: UInt64] = [:]
    private var nextTouch: UInt64 = 1
    private var failed = false
    var onFailure: ((String) -> Void)?
    var onSceneChanged: ((Int32) -> Void)?

    override init(frame: CGRect) {
        super.init(frame: frame)
        isMultipleTouchEnabled = true
        isOpaque = true
        // GPUI content has no UIKit accessibility tree in this embedding backend.
        // Accessible interaction is provided by native sibling controls.
        isAccessibilityElement = false
        target.view = self
        let center = NotificationCenter.default
        center.addObserver(self, selector: #selector(lifecycleChanged), name: UIApplication.didBecomeActiveNotification, object: nil)
        center.addObserver(self, selector: #selector(lifecycleChanged), name: UIApplication.willResignActiveNotification, object: nil)
        center.addObserver(self, selector: #selector(memoryWarning), name: UIApplication.didReceiveMemoryWarningNotification, object: nil)
        center.addObserver(self, selector: #selector(appearanceChanged), name: UIAccessibility.reduceMotionStatusDidChangeNotification, object: nil)
    }
    required init?(coder: NSCoder) { fatalError("Use init(frame:)") }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        if window == nil {
            stop()
        } else {
            setNeedsLayout()
            startDisplayLink()
        }
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        guard window != nil, !failed else { return }
        let scale = window?.screen.scale ?? traitCollection.displayScale
        contentScaleFactor = scale
        let width = UInt32((bounds.width * scale).rounded())
        let height = UInt32((bounds.height * scale).rounded())
        if handle == 0 {
            guard width > 0, height > 0 else { return }
            handle = gpuimm_create_ios(Unmanaged.passUnretained(self).toOpaque(), width, height, Float(scale))
            guard handle != 0 else { fail("GPU initialization failed"); return }
            appearanceChanged()
            updateActivity()
        } else {
            check(gpuimm_resize(handle, width, height, Float(scale)))
        }
    }

    private func startDisplayLink() {
        guard displayLink == nil else { return }
        let link = CADisplayLink(target: target, selector: #selector(FrameTarget.frame(_:)))
        link.add(to: .main, forMode: .common)
        displayLink = link
        updateActivity()
    }

    @objc private func lifecycleChanged(_ notification: Notification) {
        updateActivity(forceInactive: notification.name == UIApplication.willResignActiveNotification)
    }
    private func updateActivity(forceInactive: Bool = false) {
        let active = !forceInactive && window != nil && UIApplication.shared.applicationState == .active
        displayLink?.isPaused = !active
        if handle != 0 { check(gpuimm_lifecycle(handle, active)) }
        if !active { touchIDs.removeAll() }
    }
    @objc private func memoryWarning() { if handle != 0 { check(gpuimm_memory_warning(handle)) } }
    @objc private func appearanceChanged() {
        if handle != 0 { check(gpuimm_appearance(handle, traitCollection.userInterfaceStyle == .dark, UIAccessibility.isReduceMotionEnabled)) }
    }
    override func traitCollectionDidChange(_ previous: UITraitCollection?) {
        super.traitCollectionDidChange(previous)
        appearanceChanged()
        setNeedsLayout()
    }

    func renderFrame() {
        guard handle != 0, !failed else { return }
        check(gpuimm_tick(handle))
        if !failed { onSceneChanged?(gpuimm_count(handle)) }
    }
    func advance() {
        guard handle != 0 else { return }
        let count = gpuimm_advance(handle)
        check(count)
        if count >= 0 { onSceneChanged?(count) }
    }
    func setTitle(_ title: String) {
        guard handle != 0 else { return }
        let bytes = Array(title.utf8)
        let result = bytes.withUnsafeBufferPointer { gpuimm_set_title(handle, $0.baseAddress, $0.count) }
        check(result)
    }
    func stop() {
        displayLink?.invalidate()
        displayLink = nil
        touchIDs.removeAll()
        if handle != 0 { gpuimm_destroy(handle); handle = 0 }
    }
    private func check(_ code: Int32) { if code < 0 { fail("GPU operation failed; see the native log") } }
    private func fail(_ message: String) {
        failed = true
        stop()
        onFailure?(message)
    }

    private func send(_ touches: Set<UITouch>, phase: UInt32) {
        guard handle != 0 else { return }
        for touch in touches {
            let key = ObjectIdentifier(touch)
            if phase == 0 { touchIDs[key] = nextTouch; nextTouch &+= 1 }
            guard let id = touchIDs[key] else { continue }
            let location = touch.location(in: self)
            check(gpuimm_touch(handle, id, phase, Float(location.x), Float(location.y)))
            if phase >= 2 { touchIDs.removeValue(forKey: key) }
        }
    }
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) { send(touches, phase: 0) }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) { send(touches, phase: 1) }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) { send(touches, phase: 2) }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) { send(touches, phase: 3) }

    deinit {
        displayLink?.invalidate()
        if handle != 0 { gpuimm_destroy(handle) }
    }
}

/// SwiftUI owns the view's frame. Native SwiftUI controls can be siblings in a ZStack.
struct GpuimmCanvas: UIViewRepresentable {
    func makeUIView(context: Context) -> GpuimmView { GpuimmView(frame: .zero) }
    func updateUIView(_ uiView: GpuimmView, context: Context) {}
    static func dismantleUIView(_ uiView: GpuimmView, coordinator: ()) { uiView.stop() }
}
