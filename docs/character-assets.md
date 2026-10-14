# 角色、思考气泡与项目图标

角色素材：`apps/desktop/public/silverwolf.png`。

来源为用户在 2026-10-13 提供的第 3 张参考图（银狼人物）。使用内置 imagegen 编辑工具移除白底、原气泡及文字，并补齐右侧少量裁切边缘。该素材是参考图的处理版本，不声称为项目原创人物。原附件未被覆盖，项目无需联网加载图片。

气泡由 Vue / SVG / CSS 绘制，参考用户提供的第 1、2 张图：椭圆粗描边、两个思考圆点、紫色额度、低额度金色。气泡中的数值和重置时间来自实际查询；设置入口内保留跟随、缩放与 CLI 路径。

## 角色素材：内置工具使用的完整提示词

Use case: background-extraction. Edit target: the attached chibi Silver Wolf character illustration. Create a clean transparent PNG character cutout for a desktop widget. Remove ONLY the white background, the entire thought bubble including all text and its small floating outline circles, and the two small blue emphasis marks to the left. Preserve the character from the input as faithfully as possible: same sleepy sideways-looking face, silver-grey hair, violet eyes, cyan-violet pixel-pattern sunglasses resting on her head, purple-blue dotted black bow, hair clips, fur-trimmed jacket, neck accessories; same illustration strokes, colors, expression and proportions. Do not redesign or restyle. Character bust only, same framing to upper chest, NO legs or full body. Fit the entire character head and tall bow inside a tightly framed portrait canvas with very little transparent margin, centering the cutout. Where the original right edge narrowly cuts hair/bow, complete that narrow edge naturally only. True transparent alpha outside silhouette, keep white fur and highlights opaque. No text, no speech bubble, no label, no border around the asset, no drop shadow.

## 缎带项目与托盘图标

2026-10-14 使用内置 imagegen 工具生成，以 `apps/desktop/public/silverwolf.png` 为视觉参考，提取头顶缎带的尖角双叶轮廓、深靛底色及紫蓝圆点。透明原图保存于 `docs/assets/silverwolf-ribbon.png`，README 直接引用。图案衍生自角色参考，不声称为原创角色标识。

`node scripts/generate-icons.mjs` 调用已安装的 Tauri CLI 离线缩放原图，保留透明通道，输出 `apps/desktop/src-tauri/icons/icon.png`（256 × 256）及 `icon.ico`（16、20、24、32、40、48、64、128、256 像素）。脚本不会重新请求 imagegen，也不会下载依赖。Tauri 打包配置引用这两个文件，托盘通过 `app.default_window_icon()` 复用应用图标。

### 缎带图标完整提示词

```text
Use case: logo-brand
Asset type: square project icon for Silverwolf Balance Tips README and Windows system tray.
Input image 1 is a visual reference only: use ONLY the ribbon tied on top of Silver Wolf's hair, not her face, hair, goggles, clothing or any other object.
Primary request: design a polished standalone icon inspired closely by that distinctive dark navy two-pointed ribbon bow. Two broad pointed ribbon loops rise diagonally from a small central knot, one taller on the left and one angling to the right. Preserve the angular silhouette and dark indigo fabric with luminous violet-to-cyan circular dot pattern, simplifying to a few large clear dots and color areas so it reads at 16–32 pixels.
Style/medium: crisp anime cel-shaded graphic logo, clean deliberate contours, minimal shading, no photorealism. Dark outline plus a narrow pale lavender outer keyline so it remains visible on light and dark taskbars. Two short angular ribbon tails may extend below the central knot to give a compact balanced bow silhouette.
Composition: single isolated bow, centered, square image, fill about 88% of canvas with equal safe margins; no cropped points.
Scene/backdrop: true transparent alpha background, no background shape.
Constraints: ribbon only, no head, no character, no goggles, no wolf face, no badge or enclosing tile, no text, no letters, no watermark, no cast shadow, no sparkles outside ribbon. Keep edges sharp and broad shapes legible at tiny sizes.
```
