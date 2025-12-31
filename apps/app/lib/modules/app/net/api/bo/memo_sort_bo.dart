import 'package:json_annotation/json_annotation.dart';

part 'memo_sort_bo.g.dart';

@JsonSerializable()
class MemoSortBo {
  MemoSortBo(this.displaymode, this.idAndSeqMap, this.minSeq, this.maxSeq);

  int? displaymode;
  Map<String, int> idAndSeqMap;
  int minSeq;
  int maxSeq;

  factory MemoSortBo.fromJson(Map<String, dynamic> json) =>
      _$MemoSortBoFromJson(json);

  Map<String, dynamic> toJson() => _$MemoSortBoToJson(this);
}
