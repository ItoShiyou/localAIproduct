import Foundation
// Synthetic text only. This separate QA host exercises the signed helper's sandbox inheritance.
let resources = Bundle.main.resourceURL!.appendingPathComponent("resources")
let model = resources.appendingPathComponent("models/Qwen3-4B-Instruct-2507-Q4_K_M.gguf")
let engine = Bundle.main.executableURL!.deletingLastPathComponent().appendingPathComponent("minutes-summarizer")
let task = Process(); task.executableURL = engine
let input = Pipe(), output = Pipe(); task.standardInput = input; task.standardOutput = output; task.standardError = FileHandle.nullDevice
 task.arguments = ["--model", model.path, "--threads", "4", "--ctx", "2048"]
do {
    try task.run()
    input.fileHandleForWriting.write(Data("{\"id\":1,\"op\":\"count\",\"text\":\"架空の会議で日程を確認しました\"}\n".utf8))
    try input.fileHandleForWriting.close()
    let data = output.fileHandleForReading.readDataToEndOfFile(); task.waitUntilExit()
    let values = String(decoding:data,as:UTF8.self).split(separator:"\n").compactMap { try? JSONSerialization.jsonObject(with:Data($0.utf8)) as? [String:Any] }
    let passed = task.terminationStatus == 0 && values.contains { $0["type"] as? String == "count" && ($0["tokens"] as? Int ?? 0) > 0 }
    let root = FileManager.default.urls(for:.applicationSupportDirectory,in:.userDomainMask).first!.appendingPathComponent("MinutesSandboxQA")
    try FileManager.default.createDirectory(at:root,withIntermediateDirectories:true)
    try Data("{\"sandbox_helper_passed\":\(passed),\"exit\":\(task.terminationStatus)}\n".utf8).write(to:root.appendingPathComponent("result.json"))
    print("sandbox_helper_passed=\(passed), exit=\(task.terminationStatus)")
    exit(passed ? 0 : 1)
} catch { print("sandbox_helper_passed=false: process launch or file access failed"); exit(1) }
