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
    NSString *meaningPath = [[@(argv[1]) stringByDeletingLastPathComponent] stringByAppendingPathComponent:@"chinese-meanings.json"];
    controller.meanings = [NSJSONSerialization JSONObjectWithData:[NSData dataWithContentsOfFile:meaningPath] options:0 error:nil];
    check(controller.meanings.count > 120000, @"bundled bilingual dictionary");
    for (NSDictionary *entry in controller.meanings.allValues)
        check([entry[@"vi"] length] || [entry[@"en"] length], @"at least one translation present");
    for (NSString *word in @[@"图书馆", @"环境", @"经济", @"人工智能", @"旅游", @"医院"])
        check([controller.meanings[word][@"vi"] length] && [controller.meanings[word][@"en"] length], @"expanded vocabulary in both languages");
    type(controller, @"nihao");
    NSArray *originalChoices = controller.choices.copy;
    NSString *originalMarked = client.marked.copy;
    for (NSInteger mode = 0; mode <= 2; mode++) {
        [NSUserDefaults.standardUserDefaults setInteger:mode forKey:@"ChineseMeaningLanguage"];
        NSString *label = [controller candidateLabelAtIndex:0 number:1];
        check([label isEqualToString:@[@"1 你好", @"1 你好 — xin chào", @"1 你好 — hello"][mode]], @"meaning language selection");
        check([controller.choices isEqual:originalChoices] && [client.marked isEqual:originalMarked], @"meaning selection preserves composition and order");
    }
    check([controller meaningForCandidate:@"🫠"] == nil, @"unknown characters have no placeholder");
    NSDictionary *dictionary = controller.meanings;
    controller.meanings = @{@"我": @{@"en": @"I; me", @"vi": @"tôi"},
        @"学习": @{@"en": @"study", @"vi": @"học"}, @"中文": @{@"en": @"Chinese", @"vi": @"tiếng Trung"},
        @"学": @{@"en": @"learn"}, @"𠮷": @{@"en": @"lucky"}};
    check([[controller meaningForCandidate:@"我学习中文！"] isEqualToString:@"我: I · 学习: study · 中文: Chinese"], @"sentence splits into longest words and characters");
    check([[controller meaningForCandidate:@"𠮷🫠我"] isEqualToString:@"𠮷: lucky · 我: I"], @"Unicode characters and unknown segments");
    [NSUserDefaults.standardUserDefaults setInteger:1 forKey:@"ChineseMeaningLanguage"];
    check([[controller meaningForCandidate:@"我学习中文"] isEqualToString:@"我: tôi · 学习: học · 中文: tiếng Trung"], @"Vietnamese sentence annotations");
    controller.choices = @[@"🫠"];
    check([[controller candidateLabelAtIndex:0 number:1] isEqualToString:@"1 🫠"], @"unknown candidate has no empty separator");
    controller.choices = originalChoices;
    controller.meanings = dictionary;
    type(controller, @" ");
    check([client.committed isEqualToString:@"你好"], @"meanings are never inserted");
    [client.committed setString:@""];
    type(controller, @"nihao ");
    check([client.committed isEqualToString:@"你好"], @"Chinese commit via controller");
    type(controller, @"ni");
    check(controller.usingRime, @"short Pinyin follows Chinese context");
    [controller finish:controller.raw.copy client:(id)client];
    type(controller, @"nhwng");
    check([controller.language isEqualToString:@"Vietnamese"], @"switch to Vietnamese");
    check([client.marked isEqualToString:@"nhưng"], @"Telex preedit");
    type(controller, @" ");
    type(controller, @"its");
    check([client.marked isEqualToString:@"ít"], @"Vietnamese context resolves its to ít");
    type(controller, @" ");
    check([client.committed hasSuffix:@"ít"], @"Vietnamese ambiguous word commits");
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
    type(controller, @"nihao");
    NSArray *candidates = controller.choices.copy;
    check(candidates.count > 2, @"several candidates to highlight");
    check(arrow(controller, kVK_DownArrow) && controller.highlightedIndex == 1, @"down highlights the next candidate");
    check(arrow(controller, kVK_UpArrow) && controller.highlightedIndex == 0, @"up highlights the previous candidate");
    check(arrow(controller, kVK_UpArrow) && controller.highlightedIndex == candidates.count - 1, @"up wraps to the last candidate");
    check(controller.page == (candidates.count - 1) / 9, @"highlight moves the page with it");
    check(arrow(controller, kVK_DownArrow) && controller.highlightedIndex == 0, @"down wraps to the first candidate");
    arrow(controller, kVK_DownArrow);
    [client.committed setString:@""];
    type(controller, @" ");
    // A single-syllable candidate only fixes that segment; Rime keeps composing the rest.
    check([client.committed isEqualToString:candidates[1]] || [client.marked hasPrefix:candidates[1]], @"space selects the highlighted candidate");
    if (controller.raw.length) [controller finish:controller.raw.copy client:(id)client];
    type(controller, @"vieetj");
    check(!arrow(controller, kVK_DownArrow), @"arrows without candidates go to the app");
    check([client.committed hasSuffix:@"việt"], @"and commit the word first");
    check(!arrow(controller, kVK_DownArrow), @"empty composition passes arrows to app");
    puts("Controller integration checks passed");
    return 0;
}}
