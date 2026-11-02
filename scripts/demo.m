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
        [controller handleEvent:event client:(id)controller.fake];
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
static void frame(TestController *c, NSString *keys, NSString *title, NSString *dir, int index) {
    NSBitmapImageRep *bitmap = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:NULL pixelsWide:1000 pixelsHigh:540 bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO colorSpaceName:NSDeviceRGBColorSpace bytesPerRow:0 bitsPerPixel:0];
    [NSGraphicsContext saveGraphicsState];
    NSGraphicsContext.currentContext = [NSGraphicsContext graphicsContextWithBitmapImageRep:bitmap];
    [[[NSGradient alloc] initWithStartingColor:[NSColor colorWithCalibratedRed:.91 green:.52 blue:.62 alpha:1]
        endingColor:[NSColor colorWithCalibratedRed:.52 green:.56 blue:.95 alpha:1]] drawInRect:NSMakeRect(0,0,1000,540) angle:-20];
    label(@"KeyIn",40,468,40,NSColor.whiteColor);
    label(@"English · Tiếng Việt · 中文 — one input method, on-device",40,436,19,[NSColor colorWithWhite:1 alpha:.85]);
    label(title,40,380,23,NSColor.whiteColor);
    [[NSColor colorWithWhite:1 alpha:.92] setFill]; [[NSBezierPath bezierPathWithRoundedRect:NSMakeRect(32,262,936,96) xRadius:16 yRadius:16] fill];
    NSFont *font = [NSFont systemFontOfSize:34];
    NSMutableAttributedString *line = [[NSMutableAttributedString alloc] initWithString:c.fake.committed
        attributes:@{NSFontAttributeName: font, NSForegroundColorAttributeName: NSColor.blackColor}];
    [line appendAttributedString:[[NSAttributedString alloc] initWithString:c.fake.marked ?: @"" attributes:@{NSFontAttributeName: font,
        NSForegroundColorAttributeName: NSColor.blackColor, NSUnderlineStyleAttributeName: @(NSUnderlineStyleSingle)}]];
    [line drawAtPoint:NSMakePoint(60,288)];
    CGFloat caret = 60 + line.size.width + 2;
    [[NSColor systemBlueColor] setFill]; NSRectFill(NSMakeRect(caret,288,2,40));
    CGFloat markedStart = 60 + [[NSAttributedString alloc] initWithString:c.fake.committed attributes:@{NSFontAttributeName: font}].size.width;
    drawPanel(c, NSMakePoint(MIN(markedStart - 18, 990 - c.panel.frame.size.width), 252));
    label([NSString stringWithFormat:@"Keys: %@",keys],40,32,19,NSColor.whiteColor);
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
    [NSUserDefaults.standardUserDefaults setVolatileDomain:@{@"AutomaticDetection":@YES,@"Language":@1,@"Autocorrect":@YES,@"EnabledLanguages":@14,@"TabShortcuts":@YES,@"ChineseMeaningLanguage":@0,@"LearnChinese":@NO} forName:NSArgumentDomain];
    check([KeyInRime startWithSharedData:@(argv[1]) userData:@(argv[2])],@"Rime startup");
    TestController *c=[TestController alloc]; c.raw=[NSMutableString new]; c.preedit=@""; c.choices=@[]; c.rime=[KeyInRime new]; c.recentLanguages=[NSMutableArray new]; c.fake=[FakeClient new]; c.fake.committed=[NSMutableString new];
    NSArray *scenes=@[@[@"hello ",@"01  English stays English"],@[@"tieengs ",@"02  Vietnamese Telex: tieengs → tiếng"],@[@"zenmeyang ",@"03  Chinese Pinyin: zenmeyang → 怎么样"],@[@"nhlm ",@"04  Chinese initials after Chinese: nhlm → 你好了吗"],@[@"ban\t\t ",@"05  Tab cycles the word's language: EN → VI → 中"]];
    int index=0;
    for (NSArray *scene in scenes) {
        [c.fake.committed setString:@""]; NSMutableString *keys=[NSMutableString new];
        for (NSUInteger i=0;i<[scene[0] length];i++) {
            NSString *ch=[scene[0] substringWithRange:NSMakeRange(i,1)]; type(c,ch); [keys appendString:[ch isEqual:@" "]?@" [Space]":([ch isEqual:@"\t"]?@" [Tab]":ch)];
            for(int hold=0;hold<([ch isEqual:@" "]?12:([ch isEqual:@"\t"]?7:2));hold++) frame(c,keys,scene[1],@(argv[3]),index++);
        }
    }
    settingsSnapshot(@(argv[3]));
    return 0;
}}
