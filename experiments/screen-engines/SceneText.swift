struct SceneText: Codable, Equatable {
    let screen: String
    let text: String
    let revision: Int
    let viewportWidth: Int?
}
