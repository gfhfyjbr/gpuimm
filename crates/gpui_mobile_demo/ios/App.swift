import UIKit

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    func application(_ application: UIApplication, configurationForConnecting session: UISceneSession,
                     options: UIScene.ConnectionOptions) -> UISceneConfiguration {
        let configuration = UISceneConfiguration(name: "Default", sessionRole: session.role)
        configuration.delegateClass = SceneDelegate.self
        return configuration
    }
}

final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options: UIScene.ConnectionOptions) {
        guard let scene = scene as? UIWindowScene else { return }
        let window = UIWindow(windowScene: scene)
        window.rootViewController = DemoController()
        self.window = window
        window.makeKeyAndVisible()
    }
}

final class DemoController: UIViewController, UITextFieldDelegate {
    let canvas = GpuimmView(frame: .zero)
    private let count = UILabel()
    private let input = UITextField()

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        let heading = UILabel()
        heading.text = "gpuimm"
        heading.font = .preferredFont(forTextStyle: .largeTitle)
        heading.adjustsFontForContentSizeCategory = true
        input.borderStyle = .roundedRect
        input.placeholder = "Rename the GPU scene"
        input.accessibilityIdentifier = "scene-title"
        input.returnKeyType = .done
        input.delegate = self
        input.addTarget(self, action: #selector(renameScene), for: .editingChanged)
        let effect: UIVisualEffect
        if #available(iOS 26.0, *), !UIAccessibility.isReduceTransparencyEnabled {
            let glass = UIGlassEffect(style: .regular)
            glass.isInteractive = true
            effect = glass
        } else {
            effect = UIBlurEffect(style: .systemMaterial)
        }
        let bar = UIVisualEffectView(effect: effect)
        bar.layer.cornerRadius = 28
        bar.clipsToBounds = true
        let button = UIButton(type: .system)
        if #available(iOS 26.0, *) { button.configuration = .prominentGlass() }
        else { button.configuration = .filled() }
        button.configuration?.title = "Next scene"
        button.accessibilityIdentifier = "native-next"
        button.addTarget(self, action: #selector(advance), for: .touchUpInside)
        count.text = "Scene 0"
        count.accessibilityIdentifier = "native-count"
        count.font = .preferredFont(forTextStyle: .body)
        count.adjustsFontForContentSizeCategory = true
        let stack = UIStackView(arrangedSubviews: [count, button])
        stack.axis = .horizontal
        stack.spacing = 20
        stack.alignment = .center
        bar.contentView.addSubview(stack)
        for item in [heading, input, canvas, bar] { view.addSubview(item); item.translatesAutoresizingMaskIntoConstraints = false }
        stack.translatesAutoresizingMaskIntoConstraints = false
        let safe = view.safeAreaLayoutGuide
        NSLayoutConstraint.activate([
            heading.leadingAnchor.constraint(equalTo: safe.leadingAnchor, constant: 24),
            heading.topAnchor.constraint(equalTo: safe.topAnchor, constant: 12),
            input.topAnchor.constraint(equalTo: heading.bottomAnchor, constant: 12),
            input.leadingAnchor.constraint(equalTo: safe.leadingAnchor, constant: 24),
            input.trailingAnchor.constraint(equalTo: safe.trailingAnchor, constant: -24),
            input.heightAnchor.constraint(greaterThanOrEqualToConstant: 44),
            canvas.topAnchor.constraint(equalTo: input.bottomAnchor, constant: 16),
            canvas.leadingAnchor.constraint(equalTo: view.leadingAnchor), canvas.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            canvas.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            bar.centerXAnchor.constraint(equalTo: safe.centerXAnchor),
            bar.bottomAnchor.constraint(equalTo: view.keyboardLayoutGuide.topAnchor, constant: -16),
            bar.leadingAnchor.constraint(greaterThanOrEqualTo: safe.leadingAnchor, constant: 16),
            stack.leadingAnchor.constraint(equalTo: bar.contentView.leadingAnchor, constant: 20),
            stack.trailingAnchor.constraint(equalTo: bar.contentView.trailingAnchor, constant: -12),
            stack.topAnchor.constraint(equalTo: bar.contentView.topAnchor, constant: 12),
            stack.bottomAnchor.constraint(equalTo: bar.contentView.bottomAnchor, constant: -12),
            button.heightAnchor.constraint(greaterThanOrEqualToConstant: 44),
        ])
        canvas.onSceneChanged = { [weak self] value in self?.count.text = "Scene \(value)" }
        canvas.onFailure = { [weak self] message in self?.count.text = message }
    }
    @objc private func advance() { canvas.advance() }
    @objc private func renameScene() { canvas.setTitle(input.text ?? "") }
    func textFieldShouldReturn(_ textField: UITextField) -> Bool { textField.resignFirstResponder(); return true }
}
