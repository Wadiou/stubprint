from fontTools.ttLib import TTFont
from fontTools.varLib import instancer
from fontTools import subset

chars = set()
for lead in range(0xa1, 0xf5):                 # JIS X 0208 rows 1 to 84 as EUC-JP
    for trail in range(0xa1, 0xff):
        try:
            chars.add(bytes([lead, trail]).decode("euc_jp"))
        except UnicodeDecodeError:
            pass
chars |= {chr(c) for c in range(0xff61, 0xffa0)}   # half width katakana
chars |= {chr(c) for c in range(0x20, 0x7f)}
font = TTFont("NotoSansJP.ttf")
font = instancer.instantiateVariableFont(font, {"wght": 400})
options = subset.Options()
options.layout_features = ["*"]
options.notdef_outline = True
subsetter = subset.Subsetter(options)
subsetter.populate(unicodes=[ord(c) for c in chars])
subsetter.subset(font)
font.save("NotoSansJP_400Regular_subset.ttf")
print(len(chars))
