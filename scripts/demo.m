// Record real controller output without changing the system input source.
#define main keyin_application_main
#import "../native/main.m"
#undef main

@interface FakeClient : NSObject
@property NSString *marked;
@property NSMutableString *committed;
@end
@implementation FakeClient
- (void)setMarkedText:(id)text selectionRange:(NSRange)selection replacementRange:(NSRange)replacement { self.marked = text; }
- (void)insertText:(id)text replacementRange:(NSRange)range { [self.committed appendString:text]; self.marked = @""; }
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
        NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:ch charactersIgnoringModifiers:ch isARepeat:NO keyCode:[ch isEqualToString:@" "] ? kVK_Space : 0];
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
static void frame(TestController *c, NSString *keys, NSString *title, NSString *dir, int index) {
    NSBitmapImageRep *bitmap = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:NULL pixelsWide:1000 pixelsHigh:540 bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO colorSpaceName:NSDeviceRGBColorSpace bytesPerRow:0 bitsPerPixel:0];
    [NSGraphicsContext saveGraphicsState];
    NSGraphicsContext.currentContext = [NSGraphicsContext graphicsContextWithBitmapImageRep:bitmap];
    [[NSColor colorWithCalibratedRed:.06 green:.09 blue:.15 alpha:1] setFill]; NSRectFill(NSMakeRect(0,0,1000,540));
    label(@"KeyIn",40,460,44,NSColor.whiteColor);
    label(@"English / Vietnamese / Chinese · on-device",40,420,22,NSColor.lightGrayColor);
    label(title,40,363,25,NSColor.whiteColor);
    [[NSColor colorWithWhite:1 alpha:1] setFill]; [[NSBezierPath bezierPathWithRoundedRect:NSMakeRect(32,130,936,210) xRadius:14 yRadius:14] fill];
    label([NSString stringWithFormat:@"Keys: %@",keys],55,290,22,NSColor.grayColor);
    NSString *output = [c.fake.committed stringByAppendingString:c.fake.marked ?: @""];
    label(output,55,226,36,NSColor.blackColor);
    NSString *candidates = c.choices.count ? [[c.choices subarrayWithRange:NSMakeRange(0,MIN(4,c.choices.count))] componentsJoinedByString:@"    "] : @"";
    label(candidates,55,175,24,NSColor.darkGrayColor);
    label([NSString stringWithFormat:@"AUTO  ·  %@  ·  Space selects / commits",c.language ?: @"English"],40,83,21,[NSColor colorWithCalibratedRed:.4 green:.85 blue:.8 alpha:1]);
    label(@"Recorded engine output · real KeyIn controller + Rime · demo layout",40,35,17,NSColor.lightGrayColor);
    [NSGraphicsContext restoreGraphicsState];
    [[bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}] writeToFile:[dir stringByAppendingPathComponent:[NSString stringWithFormat:@"%04d.png",index]] atomically:YES];
}
int main(int argc,const char **argv) { @autoreleasepool {
    check(argc==4,@"shared data, user data, frames directory required");
    [NSUserDefaults.standardUserDefaults setVolatileDomain:@{@"AutomaticDetection":@YES,@"Language":@1,@"Autocorrect":@YES} forName:NSArgumentDomain];
    check([KeyInRime startWithSharedData:@(argv[1]) userData:@(argv[2])],@"Rime startup");
    TestController *c=[TestController alloc]; c.raw=[NSMutableString new]; c.preedit=@""; c.choices=@[]; c.rime=[KeyInRime new]; c.recentLanguages=[NSMutableArray new]; c.fake=[FakeClient new]; c.fake.committed=[NSMutableString new];
    NSArray *scenes=@[@[@"hello ",@"01  English stays English"],@[@"tieengs ",@"02  Vietnamese Telex: tieengs → tiếng"],@[@"zenmeyang ",@"03  Chinese Pinyin: zenmeyang → 怎么样"],@[@"nhlm ",@"04  Chinese initials after Chinese: nhlm → 你好了吗"]];
    int index=0;
    for (NSArray *scene in scenes) {
        [c.fake.committed setString:@""]; NSMutableString *keys=[NSMutableString new];
        for (NSUInteger i=0;i<[scene[0] length];i++) {
            NSString *ch=[scene[0] substringWithRange:NSMakeRange(i,1)]; type(c,ch); [keys appendString:[ch isEqual:@" "]?@" [Space]":ch];
            for(int hold=0;hold<([ch isEqual:@" "]?12:2);hold++) frame(c,keys,scene[1],@(argv[3]),index++);
        }
    }
    return 0;
}}
