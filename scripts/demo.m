// Record real controller output without changing the system input source.
#define main keyin_application_main
#import "../native/main.m"
#undef main
#import <objc/runtime.h>

@interface FakeClient : NSObject
@property NSString *marked;
@property NSMutableString *committed;
@end
@implementation FakeClient
- (void)setMarkedText:(id)text selectionRange:(NSRange)selection replacementRange:(NSRange)replacement { self.marked = text; }
- (void)insertText:(id)text replacementRange:(NSRange)range { [self.committed appendString:text]; self.marked = @""; }
- (NSDictionary *)attributesForCharacterIndex:(NSUInteger)index lineHeightRectangle:(NSRect *)rect { if (rect) *rect = NSMakeRect(0, 0, 2, 20); return @{}; }
@end
@interface TestController : KeyInController
@property FakeClient *fake;
@end
@implementation TestController
- (void)showCandidates:(id)client {}
- (id)client { return self.fake; }
@end
static void check(BOOL condition, NSString *message) {
    if (!condition) { fprintf(stderr, "FAIL: %s\n", message.UTF8String); exit(1); }
}
static void type(TestController *controller, NSString *input) {
    for (NSUInteger i = 0; i < input.length; i++) {
        NSString *ch = [input substringWithRange:NSMakeRange(i, 1)];
        unsigned short code = [ch isEqualToString:@" "] ? kVK_Space : ([ch isEqualToString:@"\t"] ? kVK_Tab : 0);
        NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:ch charactersIgnoringModifiers:ch isARepeat:NO keyCode:code];
        // Keys the input method passes through (punctuation) reach the app as typed.
        if (![controller handleEvent:event client:(id)controller.fake] && code != kVK_Tab) [controller.fake.committed appendString:ch];
    }
}
static BOOL arrow(TestController *controller, unsigned short key) {
    NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:@"" charactersIgnoringModifiers:@"" isARepeat:NO keyCode:key];
    return [controller handleEvent:event client:(id)controller.fake];
}
static void label(NSString *text, CGFloat x, CGFloat y, CGFloat size, NSColor *color) {
    [text drawAtPoint:NSMakePoint(x,y) withAttributes:@{NSFontAttributeName:[NSFont systemFontOfSize:size], NSForegroundColorAttributeName:color}];
}
// Lay out the real candidate panel offscreen, then paint its views into the frame.
static void drawPanel(KeyInController *c, NSPoint topLeft) {
    IMP show = method_getImplementation(class_getInstanceMethod(KeyInController.class, @selector(showCandidates:)));
    ((void (*)(id, SEL, id))show)(c, @selector(showCandidates:), c.client);
    [c.panel orderOut:nil];
    NSView *panel = c.panel.contentView;
    NSRect box = NSMakeRect(topLeft.x, topLeft.y - panel.bounds.size.height, panel.bounds.size.width, panel.bounds.size.height);
    [NSGraphicsContext saveGraphicsState];
    NSShadow *shadow = [NSShadow new];
    shadow.shadowColor = [NSColor colorWithWhite:0 alpha:.22]; shadow.shadowBlurRadius = 18; shadow.shadowOffset = NSMakeSize(0, -6);
    [shadow set];
    [[NSColor colorWithWhite:.975 alpha:.96] setFill];
    [[NSBezierPath bezierPathWithRoundedRect:box xRadius:14 yRadius:14] fill];
    [NSGraphicsContext restoreGraphicsState];
    for (NSView *view in panel.subviews) {
        if (view.hidden) continue;
        NSRect rect = NSOffsetRect(view.frame, box.origin.x, box.origin.y);
        if ([view isKindOfClass:NSBox.class]) { [[NSColor colorWithWhite:0 alpha:.1] setFill]; NSRectFill(rect); continue; }
        if (view.layer.backgroundColor) {
            [[NSColor colorWithCGColor:view.layer.backgroundColor] setFill];
            [[NSBezierPath bezierPathWithRoundedRect:rect xRadius:view.layer.cornerRadius yRadius:view.layer.cornerRadius] fill];
        }
        NSBitmapImageRep *rep = [view bitmapImageRepForCachingDisplayInRect:view.bounds];
        [view cacheDisplayInRect:view.bounds toBitmapImageRep:rep];
        [rep drawInRect:rect fromRect:NSZeroRect operation:NSCompositingOperationSourceOver fraction:1 respectFlipped:YES hints:nil];
    }
}
// One long mixed-language sentence in a plain document window; the real panel sits under the caret.
static void frame(TestController *c, NSString *keys, NSString *dir, int index) {
    const CGFloat W = 1000, H = 800;
    NSBitmapImageRep *bitmap = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:NULL pixelsWide:W pixelsHigh:H bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO colorSpaceName:NSDeviceRGBColorSpace bytesPerRow:0 bitsPerPixel:0];
    [NSGraphicsContext saveGraphicsState];
    NSGraphicsContext.currentContext = [NSGraphicsContext graphicsContextWithBitmapImageRep:bitmap];
    [[NSColor colorWithCalibratedRed:.93 green:.93 blue:.96 alpha:1] setFill]; NSRectFill(NSMakeRect(0,0,W,H));
    label(@"KeyIn",32,748,30,NSColor.blackColor);
    label(@"English · Tiếng Việt · 中文 in one sentence — no switching keys, Auto detection per word",32,722,16,NSColor.darkGrayColor);
    NSRect card = NSMakeRect(24,64,W-48,636);
    [[NSColor whiteColor] setFill]; [[NSBezierPath bezierPathWithRoundedRect:card xRadius:12 yRadius:12] fill];
    [[NSColor colorWithWhite:0 alpha:.12] setStroke]; [[NSBezierPath bezierPathWithRoundedRect:NSInsetRect(card,.5,.5) xRadius:12 yRadius:12] stroke];
    NSFont *font = [NSFont systemFontOfSize:28];
    NSMutableParagraphStyle *style = [NSMutableParagraphStyle new]; style.lineSpacing = 10;
    NSMutableAttributedString *text = [[NSMutableAttributedString alloc] initWithString:c.fake.committed
        attributes:@{NSFontAttributeName: font, NSForegroundColorAttributeName: NSColor.blackColor, NSParagraphStyleAttributeName: style}];
    NSUInteger markedStart = text.length;
    [text appendAttributedString:[[NSAttributedString alloc] initWithString:c.fake.marked ?: @"" attributes:@{NSFontAttributeName: font,
        NSForegroundColorAttributeName: NSColor.blackColor, NSUnderlineStyleAttributeName: @(NSUnderlineStyleSingle), NSParagraphStyleAttributeName: style}]];
    // Lay the text out to find where the composition starts and where the caret ends.
    NSTextStorage *storage = [[NSTextStorage alloc] initWithAttributedString:text];
    NSLayoutManager *layout = [NSLayoutManager new]; [storage addLayoutManager:layout];
    NSTextContainer *container = [[NSTextContainer alloc] initWithSize:NSMakeSize(card.size.width - 56, CGFLOAT_MAX)];
    container.lineFragmentPadding = 0; [layout addTextContainer:container];
    [layout ensureLayoutForTextContainer:container];
    CGFloat top = NSMaxY(card) - 30;
    NSRect used = [layout usedRectForTextContainer:container];
    NSImage *image = [NSImage imageWithSize:NSMakeSize(container.size.width, MAX(used.size.height, 1)) flipped:YES drawingHandler:^BOOL(NSRect r) {
        [layout drawGlyphsForGlyphRange:NSMakeRange(0, layout.numberOfGlyphs) atPoint:NSZeroPoint]; return YES; }];
    [image drawAtPoint:NSMakePoint(card.origin.x + 28, top - image.size.height) fromRect:NSZeroRect operation:NSCompositingOperationSourceOver fraction:1];
    NSRect caretLine = storage.length ? [layout lineFragmentRectForGlyphAtIndex:layout.numberOfGlyphs - 1 effectiveRange:NULL] : NSMakeRect(0,0,0,34);
    NSPoint end = storage.length ? [layout locationForGlyphAtIndex:layout.numberOfGlyphs - 1] : NSZeroPoint;
    CGFloat caretX = card.origin.x + 28 + (storage.length ? NSMaxX([layout boundingRectForGlyphRange:NSMakeRange(layout.numberOfGlyphs - 1, 1) inTextContainer:container]) : 0);
    (void)end;
    CGFloat lineTop = top - caretLine.origin.y, lineBottom = top - NSMaxY(caretLine);
    [[NSColor systemBlueColor] setFill]; NSRectFill(NSMakeRect(caretX + 1, lineBottom + 10, 2, lineTop - lineBottom - 14));
    if (c.raw.length) {
        NSUInteger glyph = [layout glyphIndexForCharacterAtIndex:MIN(markedStart, storage.length - 1)];
        CGFloat markX = card.origin.x + 28 + [layout boundingRectForGlyphRange:NSMakeRange(glyph, 1) inTextContainer:container].origin.x;
        drawPanel(c, NSMakePoint(MIN(markX - 18, W - 12 - c.panel.frame.size.width), lineBottom + 4));
    }
    NSString *recent = keys.length > 60 ? [@"…" stringByAppendingString:[keys substringFromIndex:keys.length - 60]] : keys;
    label([NSString stringWithFormat:@"Keys: %@", recent],32,24,17,NSColor.darkGrayColor);
    [NSGraphicsContext restoreGraphicsState];
    [[bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}] writeToFile:[dir stringByAppendingPathComponent:[NSString stringWithFormat:@"%04d.png",index]] atomically:YES];
}
static void settingsSnapshot(NSString *dir) {
    KeyInSettings *settings = KeyInSettings.shared;
    [settings show];
    NSView *view = settings.window.contentView;
    view.wantsLayer = YES; view.layer.backgroundColor = NSColor.windowBackgroundColor.CGColor;
    [view layoutSubtreeIfNeeded];
    NSBitmapImageRep *rep = [view bitmapImageRepForCachingDisplayInRect:view.bounds];
    [view cacheDisplayInRect:view.bounds toBitmapImageRep:rep];
    [settings.window orderOut:nil];
    [[rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}] writeToFile:[dir stringByAppendingPathComponent:@"settings.png"] atomically:YES];
}
int main(int argc,const char **argv) { @autoreleasepool {
    check(argc==4,@"shared data, user data, frames directory required");
    [NSApplication sharedApplication];
    [NSUserDefaults.standardUserDefaults setVolatileDomain:@{@"AutomaticDetection":@YES,@"Language":@1,@"Autocorrect":@YES,@"EnabledLanguages":@14,@"TabShortcuts":@YES,@"ChineseMeaningLanguage":@2,@"LearnChinese":@NO} forName:NSArgumentDomain];
    check([KeyInRime startWithSharedData:@(argv[1]) userData:@(argv[2])],@"Rime startup");
    TestController *c=[TestController alloc]; c.raw=[NSMutableString new]; c.preedit=@""; c.choices=@[]; c.rime=[KeyInRime new]; c.recentLanguages=[NSMutableArray new]; c.fake=[FakeClient new]; c.fake.committed=[NSMutableString new];
    // "|" marks a pause so the candidates (and Chinese meanings) can be read before Space.
    NSString *sentence=@"Hoom nay tooi hojc tieengs Trung voiws my friend. Teacher nois nihao|  nghiax laf hello, xiexie|  nghiax laf thank you. Cuoois tuaanf, women| yiqi| qu chifan|  nhes!";
    // The demo binary has no app bundle, so load the meanings dictionary from the source tree.
    c.meanings=[NSJSONSerialization JSONObjectWithData:[NSData dataWithContentsOfFile:@"data/chinese-meanings.json"] options:0 error:nil];
    check(c.meanings.count>0,@"Chinese meanings loaded");
    NSMutableString *keys=[NSMutableString new];
    int index=0;
    for (int hold=0;hold<8;hold++) frame(c,keys,@(argv[3]),index++);
    for (NSUInteger i=0;i<sentence.length;i++) {
        NSString *ch=[sentence substringWithRange:NSMakeRange(i,1)];
        if ([ch isEqual:@"|"]) { for(int hold=0;hold<14;hold++) frame(c,keys,@(argv[3]),index++); continue; }
        type(c,ch); [keys appendString:ch];
        for(int hold=0;hold<([ch isEqual:@" "]?3:1);hold++) frame(c,keys,@(argv[3]),index++);
    }
    for (int hold=0;hold<30;hold++) frame(c,keys,@(argv[3]),index++);
    settingsSnapshot(@(argv[3]));
    return 0;
}}
