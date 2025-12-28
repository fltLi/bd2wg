open System
open System.IO
open System.Text.RegularExpressions

let filename = "./ansy.xml"

/// 打乱归档条目
let shuffleWorks (works: string list) : string =
    // 保留最后一块, 打乱其余块
    let last = List.last works
    let init = works |> List.take (List.length works - 1)
    let rnd = System.Random()
    let shuffled = init |> List.sortBy (fun _ -> rnd.Next())
    
    // 重新组合
    System.String.Join(Environment.NewLine + Environment.NewLine, shuffled @ [last])

/// 将连续两个及以上的空行缩减为一个
let normalizeEmptyLines (text: string) : string =
    let pattern = @"(\r?\n\s*){4,}"
    let replacement = Environment.NewLine + Environment.NewLine
    Regex.Replace(text, pattern, replacement)

/// 读取文件并处理
let processFile (path: string) =
    // 读取原始文本
    let content = File.ReadAllText path
    let startTag, endTag = "<works>", "</works>"

    // 定位标签位置
    let s = content.IndexOf(startTag, StringComparison.Ordinal)
    let e = content.IndexOf(endTag, StringComparison.Ordinal)
    if s < 0 || e < 0 || s >= e then
        printfn "invalid format"
    else
        // 拆分为三段: 标签前 + 中间 + 标签及其后
        let before = content.Substring(0, s + startTag.Length)
        let middle = content.Substring(s + startTag.Length, e - (s + startTag.Length))
        let after = content.Substring(e)

        // 按空行分块, 去掉空块
        let works =
            Regex.Split(middle.Trim('\n'), "\r?\n\s*\r?\n")
            |> Array.choose (fun b -> if String.IsNullOrWhiteSpace b then None else Some b)
            |> Array.toList

        if List.isEmpty works then printfn "no work." 
        else
            // 打乱并重组
            let middle = shuffleWorks works
            let text = before + "\n" + middle + "\n" + after |> normalizeEmptyLines
            
            File.WriteAllText(path, text)
            printfn "done! shuffle %d works." (List.length works)

try
    processFile filename
with ex -> printfn "error: %s" ex.Message
