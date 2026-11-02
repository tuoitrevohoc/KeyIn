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
int main(int argc, const char **argv) { @autoreleasepool {
    check(argc == 3, @"arguments");
    [NSUserDefaults.standardUserDefaults setVolatileDomain:@{@"AutomaticDetection": @YES, @"Language": @1, @"Autocorrect": @YES} forName:NSArgumentDomain];
    check([KeyInRime startWithSharedData:@(argv[1]) userData:@(argv[2])], @"Rime startup");
    FakeClient *client = [FakeClient new]; client.committed = [NSMutableString new];
    // InputMethodKit requires a private client proxy in its constructor. This
    // harness initializes only KeyIn state and drives the real event handlers.
    TestController *controller = [TestController alloc];
    controller.raw = [NSMutableString new]; controller.preedit = @""; controller.choices = @[];
    controller.rime = [KeyInRime new]; controller.recentLanguages = [NSMutableArray new];
    controller.fake = client;
    check(controller != nil, @"controller init");
    type(controller, @"nihao ");
    check([client.committed isEqualToString:@"你好"], @"Chinese commit via controller");
    type(controller, @"ni");
    check(controller.usingRime, @"short Pinyin follows Chinese context");
    [controller finish:controller.raw.copy client:(id)client];
    type(controller, @"nhwng");
    check([controller.language isEqualToString:@"Vietnamese"], @"switch to Vietnamese");
    check([client.marked isEqualToString:@"nhưng"], @"Telex preedit");
    type(controller, @" ");
    type(controller, @"hello");
    check([controller.language isEqualToString:@"English"], @"switch to English");
    [controller finish:controller.raw.copy client:(id)client];
    type(controller, @"zenmeyang");
    check(controller.usingRime, @"switch back to Rime");
    check([controller.choices.firstObject isEqualToString:@"怎么样"], @"Rime choices reach UI");
    type(controller, @" ");
    check([client.committed hasSuffix:@"怎么样"], @"second Chinese commit");
    // Latest Chinese selection must outweigh older English/Vietnamese commits.
    controller.recentLanguages = [@[@1, @1, @3] mutableCopy];
    controller.language = @"English"; // A transient/empty refresh cannot erase context.
    type(controller, @"n");
    check(controller.usingRime, @"Chinese prefix retains last committed language");
    type(controller, @"e");
    check(controller.usingRime, @"ne continues Chinese");
    check([controller.choices.firstObject isEqualToString:@"呢"], @"ne ranks 呢 first");
    type(controller, @" ");
    check([client.committed hasSuffix:@"怎么样呢"], @"zenmeyang then ne commit");
    type(controller, @"nhlm");
    check(controller.usingRime, @"initials retain Chinese context");
    check([controller.choices.firstObject isEqualToString:@"你好了吗"], @"initials candidate reaches UI");
    type(controller, @" ");
    check([client.committed hasSuffix:@"你好了吗"], @"initials commit");
    type(controller, @"hello");
    check([controller.language isEqualToString:@"English"], @"strong English still switches");
    check(controller.recentLanguages.count <= 3, @"bounded context");
    [controller finish:controller.raw.copy client:(id)client];
    [controller.recentLanguages removeAllObjects]; controller.language = @"English";
    type(controller, @"chao");
    check(controller.languageOptions.count == 3, @"three language interpretations");
    check([controller.language isEqualToString:@"Vietnamese"], @"initial interpretation");
    check(arrow(controller, kVK_DownArrow), @"down selects language");
    check(controller.usingRime && controller.wordMode == 3, @"down enters Chinese");
    check([controller.raw isEqualToString:@"chao"], @"switch retains original keys");
    check(arrow(controller, kVK_UpArrow), @"up selects previous language");
    check([controller.language isEqualToString:@"Vietnamese"], @"up returns to Vietnamese");
    arrow(controller, kVK_UpArrow);
    check([controller.language isEqualToString:@"English"], @"up selects literal English");
    type(controller, @" ");
    check([client.committed hasSuffix:@"chao"], @"selected interpretation commits");
    check(controller.wordMode == 0 && controller.mode == 0, @"selection is per word, Auto persists");
    check(controller.recentLanguages.lastObject.unsignedIntValue == 1, @"chosen language updates context");
    check(!arrow(controller, kVK_DownArrow), @"empty composition passes arrows to app");
    type(controller, @"chao");
    arrow(controller, kVK_DownArrow);
    arrow(controller, kVK_DownArrow);
    arrow(controller, kVK_DownArrow);
    check([controller.language isEqualToString:@"Vietnamese"], @"language selection wraps");
    puts("Controller integration checks passed");
    return 0;
}}
